// ============ 导入 ============
// 负责：zip 导入、局域网接收导入、导入取消、接收缓存清理。
// 短语/笔记均复用各自的合并逻辑（english::phrase / english::note）。

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use uuid;
use zip::ZipArchive;

use crate::core::paths::{device_dir_of, get_subject_dir, module_of_file, EXPORTABLE_DIRS};
use crate::core::receive::{ReceivedFile, ReceiveState};
#[cfg(target_os = "android")]
use crate::core::saf::content_uri_to_cache;

/// 导入状态：取消标志
pub struct ImportState {
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Serialize)]
struct DuplicateInfo {
    keyword: String,
    new_list: String,
    old_list: String,
    source_file: String,
}

#[tauri::command]
pub async fn import_subject_zip(
    app: AppHandle,
    zip_path: String,
    import_state: State<'_, ImportState>,
) -> Result<serde_json::Value, String> {
    // 获取app路径
    let subject_dir = get_subject_dir(&app)?;
    fs::create_dir_all(&subject_dir).map_err(|e| e.to_string())?;
    // 克隆cancel引用，但是指向了同一块内存
    let cancel = import_state.cancel.clone();
    // 更改变量为false
    cancel.store(false, Ordering::SeqCst);

    // ★ 安卓 SAF：open() 返回的是 content:// URI，先拷到应用缓存变成真实路径
    #[cfg(target_os = "android")]
    let zip_path = if zip_path.starts_with("content://") {
        content_uri_to_cache(&app, &zip_path, "import_tmp.zip")?
            .to_string_lossy()
            .into_owned()
    } else {
        zip_path
    };

    let file = fs::File::open(&zip_path).map_err(|e| format!("打开 zip 失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("不是有效的 zip: {}", e))?;

    // 先把所有条目读进内存（db 文件很小），便于按 manifest 派发到不同导入函数
    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    let mut manifest_bytes: Option<Vec<u8>> = None;

    // ★ 安卓上 /tmp 不存在且不可写，统一改用应用缓存目录
    let tmp_root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&tmp_root).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }

        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }

        // zip 内的相对路径，统一用 /
        let name = entry.name().replace('\\', "/");

        // manifest.json 单独保存，稍后解析
        if name == "manifest.json" {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            manifest_bytes = Some(buf);
            continue;
        }

        // 只处理 .db
        if !name.ends_with(".db") {
            continue;
        }

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        entries.insert(name, buf);
    }

    // 解析 manifest；若无则回退到「按表嗅探」（兼容无 manifest 的旧备份）
    let mut tasks: Vec<(String, String, Vec<u8>)> = Vec::new(); // (module, path, bytes)
    if let Some(mb) = manifest_bytes {
        match serde_json::from_slice::<serde_json::Value>(&mb) {
            Ok(v) => {
                if let Some(items) = v.get("items").and_then(|x| x.as_array()) {
                    for it in items {
                        let module = it
                            .get("module")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let path = it
                            .get("path")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        if let Some(bytes) = entries.get(&path) {
                            tasks.push((module, path, bytes.clone()));
                        }
                    }
                }
            }
            Err(e) => return Err(format!("manifest.json 解析失败: {}", e)),
        }
    } else {
        // 回退：嗅探每个 .db 的表，自动判断模块
        for (name, bytes) in &entries {
            let tmp = tmp_root.join(format!("import_sniff_{}.db", uuid::Uuid::new_v4()));
            if fs::write(&tmp, bytes).is_err() {
                continue;
            }
            if let Some(module) = module_of_file(&tmp) {
                tasks.push((module.to_string(), name.clone(), bytes.clone()));
            }
            let _ = fs::remove_file(&tmp);
        }
    }

    let mut duplicates: Vec<DuplicateInfo> = Vec::new();
    let mut imported = 0i64;
    let mut skipped = 0i64;

    for (module, name, bytes) in tasks {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }
        match module.as_str() {
            // 短语：复用原合并逻辑（按 keyword 查重，重复项交给前端弹窗处理）
            "phrase" => {
                import_phrase_bytes(
                    &bytes,
                    &subject_dir,
                    &tmp_root,
                    &name,
                    &cancel,
                    &mut imported,
                    &mut skipped,
                    &mut duplicates,
                )?;
            }
            // 笔记：不查重，目录重命名为「导入目录-原名称」后整体并入
            "note" => match crate::english::note::import_note_bytes(
                &bytes,
                &app,
                "导入目录",
                &mut imported,
                &mut skipped,
            ) {
                Ok(()) => {}
                Err(e) => {
                    println!("[import] note 导入失败: {}", e);
                    skipped += 1;
                }
            },
            // 未知模块：跳过，不阻断其它模块
            _ => {
                println!("[import] 未知 module: {}", module);
                skipped += 1;
            }
        }
    }

    // 清理导入时落在缓存里的临时 zip
    #[cfg(target_os = "android")]
    {
        let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
        let tmp_zip = cache.join("import_tmp.zip");
        if tmp_zip.exists() {
            let _ = fs::remove_file(&tmp_zip);
        }
    }

    Ok(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "duplicates": duplicates,
    }))
}

// 短语导入：从内存字节合并进目标 phrase_db（按 keyword 查重，重复项收集到 duplicates 由前端弹窗处理）
// 目标不存在则整库复制；目标存在则逐条比对插入。保持原 import_subject_zip 的行为不变。
fn import_phrase_bytes(
    bytes: &[u8],
    subject_dir: &Path,
    tmp_root: &Path,
    name: &str,
    cancel: &Arc<AtomicBool>,
    imported: &mut i64,
    skipped: &mut i64,
    duplicates: &mut Vec<DuplicateInfo>,
) -> Result<(), String> {
    // 安全：拒绝路径穿越
    if name.contains("..") {
        *skipped += 1;
        return Ok(());
    }
    // 第一段目录名必须在白名单里（zip-slip 防护，manifest 驱动下仍保留双保险）
    let first_seg = name.split('/').next().unwrap_or("");
    if !EXPORTABLE_DIRS.contains(&first_seg) {
        *skipped += 1;
        return Ok(());
    }

    let target_path = subject_dir.join(name);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // 目标不存在：直接复制整库
    if !target_path.exists() {
        fs::write(&target_path, bytes).map_err(|e| e.to_string())?;
        *imported += 1;
        return Ok(());
    }

    // 目标存在：合并
    let tmp_path = tmp_root.join(format!("import_{}.db", uuid::Uuid::new_v4()));
    fs::write(&tmp_path, bytes).map_err(|e| e.to_string())?;

    let src_conn = match Connection::open(&tmp_path) {
        Ok(c) => c,
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("打开源 db 失败: {}", e));
        }
    };

    let table_exists: bool = src_conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='phrase_table')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !table_exists {
        drop(src_conn);
        let _ = fs::remove_file(&tmp_path);
        *skipped += 1;
        return Ok(());
    }

    let count: i64 = src_conn
        .query_row("SELECT COUNT(*) FROM phrase_table", [], |r| r.get(0))
        .unwrap_or(0);
    if count == 0 {
        drop(src_conn);
        let _ = fs::remove_file(&tmp_path);
        *skipped += 1;
        return Ok(());
    }

    let dst_conn = Connection::open(&target_path).map_err(|e| e.to_string())?;
    dst_conn
        .execute(
            "CREATE TABLE IF NOT EXISTS phrase_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT, first_char TEXT, keyword TEXT, list TEXT,
                source TEXT DEFAULT 'user', state INTEGER DEFAULT 0
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
    dst_conn
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_keyword ON phrase_table(keyword)",
            [],
        )
        .map_err(|e| e.to_string())?;

    let tx = dst_conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let mut stmt = src_conn
        .prepare("SELECT type, first_char, keyword, list, source FROM phrase_table")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut cancelled = false;

    for row in rows {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        let (t, fc, kb, list, src) = row.map_err(|e| e.to_string())?;

        // base 种子不当普通数据导入：只接收 source='user' 的行，
        // 避免把另一台设备的 base（及其 state=1 隐藏标记）当成用户数据插入。
        if src != "user" {
            continue;
        }

        // 查重探针限定 source='user'：只有目标也存在 user 行才算真正冲突；
        // 目标仅有 base 行（被覆盖或纯 base）属于「干净覆盖」，不弹窗。
        let existing_user_list: Option<String> = tx
            .query_row(
                "SELECT list FROM phrase_table WHERE keyword = ?1 AND source='user'",
                params![kb],
                |r| r.get(0),
            )
            .ok();

        match existing_user_list {
            Some(old_list) => {
                duplicates.push(DuplicateInfo {
                    keyword: kb,
                    new_list: list,
                    old_list,
                    source_file: name.to_string(),
                });
            }
            None => {
                // 插入为 user 覆盖行；若目标有对应 base 行则标记其 state=1（与 save_phrase 一致）
                tx.execute(
                    "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) VALUES (?1,?2,?3,?4,'user',0)",
                    params![t, fc, kb, list],
                )
                .map_err(|e| e.to_string())?;
                let base_exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE keyword = ?1 AND source='base')",
                        params![kb],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if base_exists {
                    tx.execute(
                        "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1",
                        params![kb],
                    )
                    .map_err(|e| e.to_string())?;
                }
                *imported += 1;
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    drop(stmt);
    drop(src_conn);
    let _ = fs::remove_file(&tmp_path);

    if cancelled {
        return Err("用户取消了导入".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn import_received_files(
    app: AppHandle,
    file_ids: Vec<String>,
    state: State<'_, ReceiveState>,
    import_state: State<'_, ImportState>,
) -> Result<serde_json::Value, String> {
    let files = state.received.lock().await.clone();
    let targets: Vec<ReceivedFile> = files
        .into_iter()
        .filter(|f| file_ids.contains(&f.id))
        .collect();

    let subject_dir = get_subject_dir(&app)?;
    fs::create_dir_all(&subject_dir).map_err(|e| e.to_string())?;

    // 与 import_subject_zip 完全一致：短语走 import_phrase_bytes，笔记走 import_note_bytes
    let cancel = import_state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);

    // 临时目录（安卓上 /tmp 不可写，统一用应用缓存目录），供 import_phrase_bytes 落地临时 db
    let tmp_root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&tmp_root).map_err(|e| e.to_string())?;

    // 短语库统一并入主 phrase_db，按其在 subject 下的标准相对路径推导
    // （供 import_phrase_bytes 的 zip-slip 校验与 duplicates.source_file 展示）
    let phrase_path = crate::english::phrase::get_db_path(&app)?;
    let phrase_name = phrase_path
        .strip_prefix(&subject_dir)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "english/phrase_db.db".to_string());

    let mut duplicates: Vec<DuplicateInfo> = Vec::new();
    let mut imported = 0i64;
    let mut skipped = 0i64;

    for file in &targets {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }

        // 读源库字节（与 zip 导入一致，先落内存再交各模块导入函数处理）
        let bytes = fs::read(&file.saved_path).map_err(|e| e.to_string())?;

        // 优先按本设备 manifest.json 的 module 派发；缺失或查不到时回退到文件自带 module
        let module = resolve_module(&app, file).unwrap_or_else(|| file.module.clone());

        match module.as_str() {
            // 笔记：不查重，目录重命名为「设备名-原名」后整体并入主笔记库
            "note" => {
                if let Err(e) = crate::english::note::import_note_bytes(
                    &bytes,
                    &app,
                    &file.from_device,
                    &mut imported,
                    &mut skipped,
                ) {
                    println!("[import] note 导入失败: {}", e);
                    skipped += 1;
                }
            }
            // 短语：与 zip 导入共用 import_phrase_bytes（整库复制 / 按 keyword 查重 / 收集 duplicates）
            "phrase" => {
                if let Err(e) = import_phrase_bytes(
                    &bytes,
                    &subject_dir,
                    &tmp_root,
                    &phrase_name,
                    &cancel,
                    &mut imported,
                    &mut skipped,
                    &mut duplicates,
                ) {
                    // 取消需向上抛出，其余错误记为跳过，不阻断其它文件
                    if e == "用户取消了导入" {
                        return Err(e);
                    }
                    println!("[import] phrase 导入失败: {}", e);
                    skipped += 1;
                }
            }
            // 未知模块：跳过，不阻断其它模块
            _ => {
                println!("[import] 未知 module: {}", file.module);
                skipped += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "imported": imported,
        "skipped": skipped,
        "duplicates": duplicates,
    }))
}

/// 按本设备目录下的 manifest.json 解析某文件应派发到的模块（phrase/note）。
/// manifest 的 items[].path 与本文件 relative_path 对齐；查不到时返回 None，
/// 调用方回退到文件自带的 module 字段（兼容旧版发送端）。
fn resolve_module(app: &AppHandle, file: &ReceivedFile) -> Option<String> {
    let subject_dir = get_subject_dir(app).ok()?;
    let manifest_path = subject_dir
        .join("_received")
        .join(&file.device)
        .join("manifest.json");
    let data = fs::read(&manifest_path).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&data).ok()?;
    let items = v.get("items").and_then(|x| x.as_array())?;
    for it in items {
        let path = it.get("path").and_then(|x| x.as_str())?;
        if path == file.relative_path {
            return it.get("module").and_then(|x| x.as_str()).map(|s| s.to_string());
        }
    }
    None
}

#[tauri::command]
pub async fn cancel_import(state: State<'_, ImportState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

// ============ 清理接收缓存 ============

#[tauri::command]
pub async fn clear_received_files(
    app: AppHandle,
    state: State<'_, ReceiveState>,
) -> Result<(), String> {
    state.received.lock().await.clear();

    let subject_dir = get_subject_dir(&app)?;
    let received_root = subject_dir.join("_received");
    if let Ok(entries) = fs::read_dir(&received_root) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(())
}

/// 仅删除指定的已接收文件（同一设备共用一个临时文件夹），
/// 用于按设备导入后清理该设备的源文件，不影响其它设备发来的文件。
/// 仅当某设备目录下已无任何剩余文件时，才删除整个设备目录（含其 manifest.json）。
#[tauri::command]
pub async fn clear_received_files_by_ids(
    file_ids: Vec<String>,
    state: State<'_, ReceiveState>,
) -> Result<(), String> {
    let mut to_delete_files: Vec<PathBuf> = Vec::new();
    let mut device_dirs: Vec<PathBuf> = Vec::new();
    {
        let mut received = state.received.lock().await;
        for id in &file_ids {
            if let Some(f) = received.iter().find(|f| &f.id == id) {
                to_delete_files.push(PathBuf::from(&f.saved_path));
                if let Some(dev_dir) = device_dir_of(Path::new(&f.saved_path)) {
                    if !device_dirs.contains(&dev_dir) {
                        device_dirs.push(dev_dir);
                    }
                }
            }
        }
        received.retain(|f| !file_ids.contains(&f.id));
    }
    for p in &to_delete_files {
        let _ = fs::remove_file(p);
    }
    // 若某设备目录已无剩余文件（received 列表里不再有该设备），则删除整个设备目录
    for dev_dir in device_dirs {
        let still_has = {
            let r = state.received.lock().await;
            r.iter()
                .any(|f| device_dir_of(Path::new(&f.saved_path)) == Some(dev_dir.clone()))
        };
        if !still_has {
            let _ = fs::remove_dir_all(&dev_dir);
        }
    }
    Ok(())
}
