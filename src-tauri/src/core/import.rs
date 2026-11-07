// ============ 导入 ============
// 负责：zip 导入、局域网接收导入、导入取消、接收缓存清理。
// 短语/笔记均复用各自的合并逻辑（english::phrase / english::note）。

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json;
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// 同批合并后的一条短语（只含 source='user' 的行）。
/// 以前合并结果要先写成临时 db、再读回字节交给写入函数重新打开扫描；
/// 现在整批留在内存里直接流转，省掉两次落盘 + 两次建表/全表扫描。
#[derive(Debug, Clone)]
struct PhraseRow {
    r#type: String,
    first_char: String,
    keyword: String,
    list: String,
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

    // 同一批 zip 里可能有多个 phrase db、或跨文件出现同一 keyword：
    // 先按目标相对路径分组收集，循环结束后统一「同批合并 → 判重写入」，
    // 保证一个 keyword 只产出一条冲突或一条静默写入。
    let mut phrase_groups: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();

    for (module, name, bytes) in tasks {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }
        match module.as_str() {
            "phrase" => {
                phrase_groups.entry(name).or_default().push(bytes);
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

    // 统一处理短语分组：同批合并 → 判重 → 冲突收集 / 静默写入
    for (name, sources) in &phrase_groups {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }
        import_phrase_group(
            name,
            sources,
            &subject_dir,
            &tmp_root,
            &cancel,
            &mut imported,
            &mut skipped,
            &mut duplicates,
        )?;
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

// 一次导入批次里，同一 keyword 可能出现在同一个 db 的多行、或跨多个 db 文件。
// 先合并成「一个 keyword 一行」的内存行集，再交给 import_phrase_rows 判重写入：
// 这样同批重复只会产出一条冲突（或一条静默写入），不会「第一行先 INSERT、
// 第二行命中刚写入的 user 行变成自己跟自己冲突」。
// 只收 source='user' 的行；list 按 phrase.trim() 去重（区分大小写，与 apply_append 一致）。
// 性能：合并结果不再写成临时库字节再读回（省掉建表、落盘、二次打开、二次全表扫描）。
fn merge_phrase_sources(
    sources: &[Vec<u8>],
    tmp_root: &Path,
    cancel: &Arc<AtomicBool>,
    skipped: &mut i64,
) -> Result<Vec<PhraseRow>, String> {
    // keyword -> (type, first_char, 合并后的 items, 已见 phrase)
    let mut merged: BTreeMap<String, (String, String, Vec<serde_json::Value>, HashSet<String>)> =
        BTreeMap::new();

    for bytes in sources {
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了导入".into());
        }
        let src_path = tmp_root.join(format!("import_src_{}.db", uuid::Uuid::new_v4()));
        if fs::write(&src_path, bytes).is_err() {
            *skipped += 1;
            continue;
        }
        let conn = match Connection::open(&src_path) {
            Ok(c) => c,
            Err(e) => {
                println!("[import] 打开源短语 db 失败: {}", e);
                *skipped += 1;
                let _ = fs::remove_file(&src_path);
                continue;
            }
        };
        // 直接 prepare 全表查询：表不存在 / 旧库列不兼容会以 Err 落到下面的 skipped 分支，
        // 不再额外做 sqlite_master 探测和 COUNT(*) 全表计数（每个源 db 少两次查询）。
        let read_rows = (|| -> Result<usize, String> {
            let mut stmt = conn
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
            let mut row_count = 0usize;
            for row in rows {
                if cancel.load(Ordering::SeqCst) {
                    return Err("用户取消了导入".into());
                }
                let (t, fc, kw, list, src) = row.map_err(|e| e.to_string())?;
                row_count += 1;
                // base 种子不当普通数据导入；空 keyword 直接丢弃
                if src != "user" || kw.is_empty() {
                    continue;
                }
                let items: Vec<serde_json::Value> =
                    serde_json::from_str(&list).unwrap_or_default();
                let entry = merged
                    .entry(kw)
                    .or_insert_with(|| (t, fc, Vec::new(), HashSet::new()));
                for item in items {
                    let phrase = item
                        .get("phrase")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if phrase.is_empty() || entry.3.contains(&phrase) {
                        continue;
                    }
                    entry.3.insert(phrase);
                    entry.2.push(item);
                }
            }
            Ok(row_count)
        })();
        drop(conn);
        let _ = fs::remove_file(&src_path);
        match read_rows {
            Ok(0) => *skipped += 1,          // 空表 / 无有效行，等价原 count==0 分支
            Ok(_) => {}
            // 取消必须向上抛，其余（旧库列不兼容 / 解析失败）只跳过这一份源 db
            Err(e) if e == "用户取消了导入" => return Err(e),
            Err(e) => {
                println!("[import] 读取源短语 db 失败，跳过: {}", e);
                *skipped += 1;
            }
        }
    }

    // BTreeMap 保证稳定排序；合并结果直接产出内存行，不再写临时库 / 读回字节。
    let mut rows = Vec::with_capacity(merged.len());
    for (kw, (t, fc, items, _)) in merged {
        let list = serde_json::to_string(&items).map_err(|e| e.to_string())?;
        rows.push(PhraseRow {
            r#type: t,
            first_char: fc,
            keyword: kw,
            list,
        });
    }
    Ok(rows)
}

/// 一组指向同一个目标库的 phrase 源：先同批合并，再统一判重写入。
fn import_phrase_group(
    name: &str,
    sources: &[Vec<u8>],
    subject_dir: &Path,
    tmp_root: &Path,
    cancel: &Arc<AtomicBool>,
    imported: &mut i64,
    skipped: &mut i64,
    duplicates: &mut Vec<DuplicateInfo>,
) -> Result<(), String> {
    let rows = merge_phrase_sources(sources, tmp_root, cancel, skipped)?;
    if rows.is_empty() {
        return Ok(());
    }
    import_phrase_rows(
        &rows,
        subject_dir,
        name,
        cancel,
        imported,
        skipped,
        duplicates,
    )
}

// 短语导入：把合并后的内存行写进目标 phrase_db（按 keyword 判重，重复项收集到 duplicates 交前端弹窗处理）
// 目标库不存在则建表后整批写入；存在则先一次性载入可见 keyword，再单事务写入。
// 性能：① 不再把合并结果落成临时 db 再打开扫描（少 2 次落盘 + 2 次全表扫描）；
//       ② 判重由「每个 keyword 一次 SELECT」改为「一次扫描目标库可见行 → HashMap」；
//       ③ INSERT 复用同一条 prepared statement，不再每行重新 prepare。
// 语义不变：判重口径、user 优先 old_list、屏蔽行不算存在、取消后已写入部分仍提交。
fn import_phrase_rows(
    rows: &[PhraseRow],
    subject_dir: &Path,
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
    let target_existed = target_path.exists();

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

    // 【判重口径】关键词「存在」= 目标库里有 user 行 或 可见 base 行(state=0)。
    // 分两条 SQL 一次扫出全部可见行的 old_list：先 user 后 base，user 行优先；
    // or_insert 不覆盖，同 keyword 多条 user 时保留首次（贴近原 LIMIT 1 的扫描顺序）。
    // state=1 的屏蔽态 base 被 WHERE 排除，等于「不算存在、不读不写」。
    // 原来每个 keyword 都要 query_row 一次，现在整批只扫两次，且都走 (source,keyword) 索引前缀。
    let mut existing: HashMap<String, String> = HashMap::new();
    for sql in [
        "SELECT keyword, list FROM phrase_table WHERE source='user'",
        "SELECT keyword, list FROM phrase_table WHERE source='base' AND state=0",
    ] {
        let mut stmt = tx.prepare(sql).map_err(|e| e.to_string())?;
        let found = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in found {
            let (kw, list) = row.map_err(|e| e.to_string())?;
            existing.entry(kw).or_insert(list);
        }
    }

    let mut ins = tx
        .prepare(
            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
             VALUES (?1,?2,?3,?4,'user',0)",
        )
        .map_err(|e| e.to_string())?;

    let mut cancelled = false;
    let mut inserted = 0i64;
    for r in rows {
        if cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        // 合并阶段已只保留 source='user' 的行，这里不需要再过滤 base
        match existing.get(&r.keyword).cloned() {
            Some(old_list) => {
                duplicates.push(DuplicateInfo {
                    keyword: r.keyword.clone(),
                    new_list: r.list.clone(),
                    old_list,
                    source_file: name.to_string(),
                });
            }
            None => {
                // 库里没有任何可见行 → 全新词，静默插入一条 user 行。
                // 同批已按 keyword 合并过，每个 keyword 只会走到这里一次。
                ins.execute(params![&r.r#type, &r.first_char, &r.keyword, &r.list])
                    .map_err(|e| e.to_string())?;
                existing.insert(r.keyword.clone(), r.list.clone());
                inserted += 1;
            }
        }
    }
    drop(ins);
    tx.commit().map_err(|e| e.to_string())?;

    // 目标库原本不存在时沿用旧口径（整库复制算 1 条）；已存在则按真正插入的词条数计
    if target_existed {
        *imported += inserted;
    } else {
        *imported += 1;
    }

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

    // 与 import_subject_zip 完全一致：短语走 import_phrase_rows，笔记走 import_note_bytes
    let cancel = import_state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);

    // 临时目录（安卓上 /tmp 不可写，统一用应用缓存目录），供 merge_phrase_sources 落地源 db 临时文件；
    // 合并结果不再落盘，直接以内存行交给 import_phrase_rows
    let tmp_root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&tmp_root).map_err(|e| e.to_string())?;

    // 短语库统一并入主 phrase_db，按其在 subject 下的标准相对路径推导
    // （供 import_phrase_rows 的 zip-slip 校验与 duplicates.source_file 展示）
    let phrase_path = crate::english::phrase::get_db_path(&app)?;
    let phrase_name = phrase_path
        .strip_prefix(&subject_dir)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "english/phrase_db.db".to_string());

    let mut duplicates: Vec<DuplicateInfo> = Vec::new();
    let mut imported = 0i64;
    let mut skipped = 0i64;

    // 接收侧所有 phrase 文件都并入同一个主短语库：先收集，循环结束后统一
    // 「同批合并 → 判重」，避免跨文件同 keyword 自己跟自己冲突。
    let mut phrase_sources: Vec<Vec<u8>> = Vec::new();

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
            // 短语：收集待合并字节，与 zip 导入共用同一套同批合并 / 判重 / 冲突裁决
            "phrase" => {
                phrase_sources.push(bytes);
            }
            // 未知模块：跳过，不阻断其它模块
            _ => {
                println!("[import] 未知 module: {}", file.module);
                skipped += 1;
            }
        }
    }

    if !phrase_sources.is_empty() {
        if let Err(e) = import_phrase_group(
            &phrase_name,
            &phrase_sources,
            &subject_dir,
            &tmp_root,
            &cancel,
            &mut imported,
            &mut skipped,
            &mut duplicates,
        ) {
            // 取消需向上抛出，其余错误记为跳过，不阻断整体返回
            if e == "用户取消了导入" {
                return Err(e);
            }
            println!("[import] phrase 导入失败: {}", e);
            skipped += 1;
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
