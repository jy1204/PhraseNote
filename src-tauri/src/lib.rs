use axum::{
    extract::{Multipart, State},
    routing::post,
    Router,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::Duration;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;
const EXPORTABLE_DIRS: &[&str] = &["english"];
pub struct ScanState {
    pub running: Arc<AtomicBool>,
    pub generation: Arc<std::sync::atomic::AtomicU64>,
}

pub struct ImportState {
    pub cancel: Arc<AtomicBool>,
}

pub struct SendState {
    pub cancel: Arc<AtomicBool>,
}

/// mDNS 广播状态：把 daemon 存起来，方便 start/stop
pub struct MdnsState {
    pub daemon: Arc<Mutex<Option<mdns_sd::ServiceDaemon>>>,
    pub service_fullname: Arc<Mutex<Option<String>>>,
}

/// 接收开关：只有接收面板打开时才允许 handle_upload 接受文件
pub struct ReceiveControl {
    pub enabled: Arc<AtomicBool>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_cwd() -> Result<String, String> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn read_cwd_file(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::read_to_string(dir.join(filename)).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_cwd_file(app: tauri::AppHandle, filename: String, content: String) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(filename), content).map_err(|e| e.to_string())
}

/// 🖥️桌面端Windows：exe所在目录 → ./data/subject/english/phrase_db.db
#[cfg(desktop)]
fn get_db_path(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;

    let dir = exe_dir.join("data").join("subject").join("english");
    let db_file = dir.join("phrase_db.db");
    println!("[Desktop] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

/// 📱手机端Android：app_data_dir/subject/english/phrase_db.db
#[cfg(mobile)]
fn get_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = data_dir.join("subject").join("english");
    let db_file = dir.join("phrase_db.db");
    println!("[Mobile] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

// ============ 设备名 ============

/// Android：通过 JNI 读 Settings.Global.DEVICE_NAME，失败退回 Build.MODEL
#[cfg(target_os = "android")]
fn get_device_name() -> String {
    use jni::objects::{JObject, JString};
    use jni::JavaVM;

    let fallback = || "我的手机".to_string();

    let ctx = ndk_context::android_context();
    let vm = match unsafe { JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) } {
        Ok(v) => v,
        Err(_) => return fallback(),
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(_) => return fallback(),
    };

    let context = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };

    // 先试 Settings.Global.DEVICE_NAME
    if let Ok(resolver) = env
        .call_method(
            &context,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .and_then(|v| v.l())
    {
        if let Ok(settings_global) = env.find_class("android/provider/Settings$Global") {
            if let Ok(key) = env.new_string("device_name") {
                let result = env.call_static_method(
                    &settings_global,
                    "getString",
                    "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
                    &[(&resolver).into(), (&key).into()],
                );
                if let Ok(val) = result.and_then(|v| v.l()) {
                    if !val.is_null() {
                        let jstr = JString::from(val);
                        let s: Option<String> = env.get_string(&jstr).ok().map(|js| js.into());
                        if let Some(s) = s {
                            if !s.is_empty() {
                                return s;
                            }
                        }
                    }
                }
            }
        }
    }

    // 退回 Build.MODEL
    if let Ok(build_class) = env.find_class("android/os/Build") {
        if let Ok(model) = env
            .get_static_field(&build_class, "MODEL", "Ljava/lang/String;")
            .and_then(|v| v.l())
        {
            if !model.is_null() {
                let jstr = JString::from(model);
                let s: Option<String> = env.get_string(&jstr).ok().map(|js| js.into());
                if let Some(s) = s {
                    if !s.is_empty() {
                        return s;
                    }
                }
            }
        }
    }

    fallback()
}

/// 桌面端：用 hostname
#[cfg(not(target_os = "android"))]
fn get_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "PhraseApp".into())
}

// 保存一条 phrase 记录
#[tauri::command(rename_all = "snake_case")]
fn save_phrase(
    app: tauri::AppHandle,
    r#type: String,
    first_char: String,
    keyboard: String,
    list: String,
) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS phrase_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT,
            first_char TEXT,
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE keyboard = ?1)",
            params![keyboard],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        return Err(format!("关键词「{}」已存在", keyboard));
    }

    conn.execute(
        "INSERT INTO phrase_table (type, first_char, keyboard, list) VALUES (?1, ?2, ?3, ?4)",
        params![r#type, first_char, keyboard, list],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn load_all_phrases(app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS phrase_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT,
            first_char TEXT,
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, type, first_char, keyboard, list FROM phrase_table ORDER BY keyboard ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,i64>(0)?,
                "type": row.get::<_,String>(1)?,
                "first_char": row.get::<_,String>(2)?,
                "keyboard": row.get::<_,String>(3)?,
                "list": row.get::<_,String>(4)?
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for r in rows {
        result.push(r.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
fn delete_phrase(app: tauri::AppHandle, keyboard: String) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "DELETE FROM phrase_table WHERE keyboard = ?1",
            params![keyboard],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("未找到关键词「{}」", keyboard));
    }

    Ok(())
}

// ============ 设备发现（mDNS） ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
}

pub struct AppState {
    pub devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    pub my_id: String,
    pub my_name: String,
    pub http_port: u16,
}

#[cfg(desktop)]
fn get_subject_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;
    Ok(exe_dir.join("data").join("subject"))
}

#[cfg(mobile)]
fn get_subject_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(data_dir.join("subject"))
}

#[tauri::command]
fn scan_subject_db_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let root = get_subject_dir(&app)?;
    let mut result = Vec::new();
    if !root.exists() {
        return Ok(result);
    }
    walk_dir(&root, &mut result).map_err(|e| e.to_string())?;
    Ok(result)
}

fn walk_dir(dir: &std::path::Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|s| s.to_str()) == Some("_received") {
                continue;
            }
            walk_dir(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("db") {
            out.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

// ============ 发送文件到指定设备 ============

#[tauri::command]
async fn send_files_to_devices(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    send_state: tauri::State<'_, SendState>,
    device_ids: Vec<String>,
) -> Result<(), String> {
    let db_files = scan_subject_db_files(app.clone())?;
    if db_files.is_empty() {
        return Err("subject 目录下没有找到 .db 文件".into());
    }

    let devices = state.devices.lock().await;
    let targets: Vec<DeviceInfo> = device_ids
        .iter()
        .filter_map(|id| devices.get(id).cloned())
        .collect();
    drop(devices);

    if targets.is_empty() {
        return Err("未选择任何有效设备".into());
    }

    let cancel = send_state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let total = targets.len() * db_files.len();
    let mut done = 0;

    for device in &targets {
        for file_path in &db_files {
            if cancel.load(Ordering::SeqCst) {
                return Err("用户取消了发送".into());
            }
            let url = format!("http://{}:{}/upload", device.ip, device.port);
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown.db".into());

            let bytes = tokio::fs::read(file_path)
                .await
                .map_err(|e| e.to_string())?;
            let part = reqwest::multipart::Part::bytes(bytes)
                .file_name(file_name.clone())
                .mime_str("application/octet-stream")
                .map_err(|e| e.to_string())?;
            let form = reqwest::multipart::Form::new()
                .text("relative_path", file_path.clone())
                .part("file", part);

            let resp = client
                .post(&url)
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("发送到 {} 失败: {}", device.name, e))?;

            if !resp.status().is_success() {
                return Err(format!(
                    "发送到 {} 失败: HTTP {}",
                    device.name,
                    resp.status()
                ));
            }

            done += 1;
            let _ = app.emit(
                "send_progress",
                serde_json::json!({
                    "current": done,
                    "total": total,
                    "device": device.name,
                    "file": file_name,
                }),
            );
        }
    }

    Ok(())
}

#[tauri::command]
async fn cancel_send(state: tauri::State<'_, SendState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

// ============ 接收 HTTP 服务器 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedFile {
    pub id: String,
    pub from_device: String,
    pub relative_path: String,
    pub saved_path: String,
}

pub struct ReceiveState {
    pub received: Arc<Mutex<Vec<ReceivedFile>>>,
}

async fn start_http_server(
    port: u16,
    save_dir: PathBuf,
    received: Arc<Mutex<Vec<ReceivedFile>>>,
    receiving: Arc<AtomicBool>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let state = HttpState {
        save_dir: save_dir.clone(),
        received: received.clone(),
        app: app.clone(),
        receiving,
    };

    let router = Router::new()
        .route("/upload", post(handle_upload))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    Ok(())
}

#[derive(Clone)]
struct HttpState {
    save_dir: PathBuf,
    received: Arc<Mutex<Vec<ReceivedFile>>>,
    app: tauri::AppHandle,
    receiving: Arc<AtomicBool>,
}

async fn handle_upload(
    State(state): State<HttpState>,
    mut multipart: Multipart,
) -> Result<String, String> {
    // 只有接收面板打开时才接受文件
    if !state.receiving.load(Ordering::SeqCst) {
        return Err("对方未打开接收面板".into());
    }

    let mut relative_path = String::new();
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "relative_path" => {
                relative_path = field.text().await.map_err(|e| e.to_string())?;
            }
            "file" => {
                file_name = field.file_name().unwrap_or("unknown.db").to_string();
                file_bytes = Some(field.bytes().await.map_err(|e| e.to_string())?.to_vec());
            }
            _ => {}
        }
    }

    let bytes = file_bytes.ok_or("未收到文件数据")?;
    let file_id = uuid::Uuid::new_v4().to_string();

    let target_dir = state.save_dir.join(&file_id);
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let save_path = target_dir.join(&file_name);
    tokio::fs::write(&save_path, &bytes)
        .await
        .map_err(|e| e.to_string())?;

    let from_device = "未知设备".to_string();

    let received_file = ReceivedFile {
        id: file_id,
        from_device,
        relative_path,
        saved_path: save_path.to_string_lossy().into_owned(),
    };

    state.received.lock().await.push(received_file.clone());

    let _ = state.app.emit("file_received", &received_file);
    Ok("OK".into())
}

#[tauri::command]
async fn get_received_files(
    state: tauri::State<'_, ReceiveState>,
) -> Result<Vec<ReceivedFile>, String> {
    let files = state.received.lock().await;
    Ok(files.clone())
}

#[tauri::command]
async fn cancel_import(state: tauri::State<'_, ImportState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[derive(Debug, Serialize)]
struct DuplicateInfo {
    keyword: String,
    new_list: String,
    old_list: String,
    source_file: String,
}

#[tauri::command]
async fn import_received_files(
    app: tauri::AppHandle,
    file_ids: Vec<String>,
    state: tauri::State<'_, ReceiveState>,
    import_state: tauri::State<'_, ImportState>,
) -> Result<serde_json::Value, String> {
    let files = state.received.lock().await.clone();
    let targets: Vec<ReceivedFile> = files
        .into_iter()
        .filter(|f| file_ids.contains(&f.id))
        .collect();

    let main_db_path = get_db_path(&app)?;
    let conn = Connection::open(&main_db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS phrase_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT, first_char TEXT, keyboard TEXT, list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    // ✅ 加索引
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_keyboard ON phrase_table(keyboard)",
        [],
    )
    .map_err(|e| e.to_string())?;

    let cancel = import_state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);

    // ✅ 开事务
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let mut duplicates: Vec<DuplicateInfo> = Vec::new();
    let mut imported = 0;

    for file in &targets {
        if cancel.load(Ordering::SeqCst) {
            // 事务里已经写入的会被 commit 保留，这里直接返回
            let _ = tx.commit();
            return Err("用户取消了导入".into());
        }
        let src_conn = Connection::open(&file.saved_path).map_err(|e| e.to_string())?;

        let table_exists: bool = src_conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='phrase_table')",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if !table_exists {
            println!("[import] 跳过无 phrase_table 的文件: {:?}", file.saved_path);
            continue;
        }

        let mut stmt = src_conn
            .prepare("SELECT type, first_char, keyboard, list FROM phrase_table")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            if cancel.load(Ordering::SeqCst) {
                let _ = tx.commit();
                return Err("用户取消了导入".into());
            }
            let (t, fc, kb, list) = row.map_err(|e| e.to_string())?;

            // 查重：走索引，O(log N)
            let existing: Option<String> = tx
                .query_row(
                    "SELECT list FROM phrase_table WHERE keyboard = ?1",
                    params![kb],
                    |r| r.get(0),
                )
                .ok();

            match existing {
                Some(old_list) => {
                    duplicates.push(DuplicateInfo {
                        keyword: kb,
                        new_list: list,
                        old_list,
                        source_file: file.relative_path.clone(),
                    });
                }
                None => {
                    // ✅ 用事务的 execute
                    tx.execute(
                        "INSERT INTO phrase_table (type, first_char, keyboard, list) VALUES (?1,?2,?3,?4)",
                        params![t, fc, kb, list],
                    ).map_err(|e| e.to_string())?;
                    imported += 1;
                }
            }
        }
    }

    // ✅ 一次性提交
    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "imported": imported,
        "duplicates": duplicates,
    }))
}

#[tauri::command(rename_all = "snake_case")]
fn resolve_duplicate_overwrite(
    app: tauri::AppHandle,
    keyword: String,
    new_list: String,
) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE phrase_table SET list = ?1 WHERE keyboard = ?2",
        params![new_list, keyword],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============ 扫描设备 ============

#[tauri::command]
async fn start_scan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    scan_state: tauri::State<'_, ScanState>,
) -> Result<(), String> {
    let my_gen = scan_state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    scan_state.running.store(true, Ordering::SeqCst);
    let running = scan_state.running.clone();
    let generation = scan_state.generation.clone();
    let devices = state.devices.clone();
    let my_id = state.my_id.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let mdns = match mdns_sd::ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("mDNS daemon 创建失败: {}", e);
                return;
            }
        };

        while running.load(Ordering::SeqCst) && generation.load(Ordering::SeqCst) == my_gen {
            devices.lock().await.clear();

            if let Ok(receiver) = mdns.browse("_phraseapp._tcp.local.") {
                let deadline = tokio::time::Instant::now() + Duration::from_millis(900);
                loop {
                    let timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if timeout.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(timeout, receiver.recv_async()).await {
                        Ok(Ok(mdns_sd::ServiceEvent::ServiceResolved(info))) => {
                            let fullname = info.get_fullname().to_string();
                            if fullname.contains(&my_id) {
                                continue;
                            }
                            if let Some(ip) = info.get_addresses().iter().find(|a| a.is_ipv4()) {
                                // 名字解析优先级：
                                // 1. props 里的 name
                                // 2. instance 里 "<uuid>|<name>" 的 name 部分
                                // 3. hostname
                                let name = info
                                    .get_properties()
                                    .get("name")
                                    .map(|v| v.val_str().to_string())
                                    .or_else(|| {
                                        fullname.split('|').nth(1).map(|s| {
                                            s.split("._phraseapp").next().unwrap_or(s).to_string()
                                        })
                                    })
                                    .unwrap_or_else(|| info.get_hostname().to_string());

                                let device = DeviceInfo {
                                    id: fullname.clone(),
                                    name,
                                    ip: ip.to_string(),
                                    port: info.get_port(),
                                };
                                devices.lock().await.insert(device.id.clone(), device);
                            }
                        }
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => {
                            println!("[scan] mDNS 接收错误: {}", e);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let _ = mdns.stop_browse("_phraseapp._tcp.local.");
            }

            let list: Vec<DeviceInfo> = devices.lock().await.values().cloned().collect();
            let _ = app2.emit("devices_update", &list);

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        let _ = mdns.shutdown();
    });

    Ok(())
}

#[tauri::command]
async fn stop_scan(scan_state: tauri::State<'_, ScanState>) -> Result<(), String> {
    scan_state.running.store(false, Ordering::SeqCst);
    Ok(())
}

// ============ mDNS 广播开关 ============

#[tauri::command]
async fn start_broadcast(
    state: tauri::State<'_, MdnsState>,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if state.daemon.lock().await.is_some() {
        return Ok(());
    }

    // 显式取本机局域网 IP，Android 上 enable_addr_auto 不可靠
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_default();
    println!("[mDNS] local_ip = {}", local_ip);

    let mdns = mdns_sd::ServiceDaemon::new().map_err(|e| e.to_string())?;
    // instance name 里带上设备名，防止 TXT 记录丢失
    let instance = format!("{}|{}", app_state.my_id, app_state.my_name);
    let props = [("name", app_state.my_name.as_str())];

    let service = mdns_sd::ServiceInfo::new(
        "_phraseapp._tcp.local.",
        &instance,
        &format!("{}.local.", app_state.my_id),
        local_ip.as_str(),
        app_state.http_port,
        &props[..],
    )
    .map_err(|e| e.to_string())?;

    let fullname = service.get_fullname().to_string();
    mdns.register(service).map_err(|e| e.to_string())?;

    *state.daemon.lock().await = Some(mdns);
    *state.service_fullname.lock().await = Some(fullname.clone());
    println!("[mDNS] 开始广播: {}", fullname);
    Ok(())
}

#[tauri::command]
async fn stop_broadcast(state: tauri::State<'_, MdnsState>) -> Result<(), String> {
    let mut daemon_slot = state.daemon.lock().await;
    if let Some(mdns) = daemon_slot.take() {
        if let Some(fullname) = state.service_fullname.lock().await.take() {
            let _ = mdns.unregister(&fullname);
            println!("[mDNS] 停止广播: {}", fullname);
        }
        let _ = mdns.shutdown();
    }
    Ok(())
}

// ============ 接收开关 ============

#[tauri::command]
async fn set_receiving(
    state: tauri::State<'_, ReceiveControl>,
    enabled: bool,
) -> Result<(), String> {
    state.enabled.store(enabled, Ordering::SeqCst);
    println!("[receive] set_receiving = {}", enabled);
    Ok(())
}

// ============ 清理接收缓存 ============

#[tauri::command]
async fn clear_received_files(
    app: tauri::AppHandle,
    state: tauri::State<'_, ReceiveState>,
) -> Result<(), String> {
    state.received.lock().await.clear();

    let subject_dir = get_subject_dir(&app)?;
    let received_root = subject_dir.join("_received");
    if let Ok(entries) = std::fs::read_dir(&received_root) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(())
}

// ============ 导出 ============

/// 递归收集某个目录下所有 .db 文件的相对路径（相对 subject 根）
fn collect_db_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<(String, PathBuf)>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_db_files(base, &path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("db") {
            // 相对 subject 的路径，统一用 /
            let rel = path
                .strip_prefix(base)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, path));
        }
    }
    Ok(())
}
// ========== Android: content:// URI 与本地文件互转 ==========
#[cfg(target_os = "android")]
fn content_uri_to_cache(
    _app: &tauri::AppHandle,
    uri: &str,
    filename: &str,
) -> Result<PathBuf, String> {
    use jni::objects::{JObject, JValue};

    // 拿到 tauri 初始化好的 JavaVM 和 Activity（Context）
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut guard = vm
    .attach_current_thread()
    .map_err(|e| e.to_string())?;
    let env = &mut *guard;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    // Uri.parse(uri)
    let juri = env.new_string(uri).map_err(|e| e.to_string())?;
    let uri_obj = env
        .call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&juri)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // activity.getContentResolver()
    let resolver = env
        .call_method(
            &activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // resolver.openInputStream(uriObj)
    let stream = env
        .call_method(
            &resolver,
            "openInputStream",
            "(Landroid/net/Uri;)Ljava/io/InputStream;",
            &[JValue::Object(&uri_obj)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // 缓存目录 = activity.getCacheDir().getAbsolutePath()
    let cache_file = env
        .call_method(&activity, "getCacheDir", "()Ljava/io/File;", &[])
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let jpath = env
        .call_method(&cache_file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;
    let cache_path: String = env
        .get_string((&jpath).into())
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into();

    let target = PathBuf::from(cache_path).join(filename);
    let mut out = fs::File::create(&target).map_err(|e| e.to_string())?;

    // 循环 stream.read(byte[65536]) 写入本地文件
    let buf = env.new_byte_array(65536).map_err(|e| e.to_string())?;
    loop {
        let n = env
            .call_method(&stream, "read", "([B)I", &[JValue::Object(&buf)])
            .map_err(|e| e.to_string())?
            .i()
            .map_err(|e| e.to_string())?;
        if n <= 0 {
            break;
        }
        let bytes = env.convert_byte_array(&buf).map_err(|e| e.to_string())?;
        use std::io::Write;
        out.write_all(&bytes[..n as usize])
            .map_err(|e| e.to_string())?;
    }
    env.call_method(&stream, "close", "()V", &[])
        .map_err(|e| e.to_string())?;

    Ok(target)
}


/// 把本地文件拷贝到 content:// URI 指向的位置（导出用）
#[cfg(target_os = "android")]
fn copy_local_to_content_uri(
    _app: &tauri::AppHandle,
    uri: &str,
    local_path: &std::path::Path,
) -> Result<(), String> {
    use jni::objects::{JObject, JValue};

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut guard = vm
    .attach_current_thread()
    .map_err(|e| e.to_string())?;
    let env = &mut *guard;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    let data = fs::read(local_path).map_err(|e| e.to_string())?;

    let juri = env.new_string(uri).map_err(|e| e.to_string())?;
    let uri_obj = env
        .call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&juri)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    let resolver = env
        .call_method(
            &activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // resolver.openOutputStream(uriObj)
    let stream = env
        .call_method(
            &resolver,
            "openOutputStream",
            "(Landroid/net/Uri;)Ljava/io/OutputStream;",
            &[JValue::Object(&uri_obj)],
        )
        .map_err(|e| e.to_string())?
        .l()
        .map_err(|e| e.to_string())?;

    // 分块写入。注意 set_byte_array_region 要 &[i8]，需把 &[u8] 转换
    let buf = env.new_byte_array(65536).map_err(|e| e.to_string())?;
    for chunk in data.chunks(65536) {
        let jbytes: &[i8] =
            unsafe { std::slice::from_raw_parts(chunk.as_ptr().cast(), chunk.len()) };
        env.set_byte_array_region(&buf, 0, jbytes)
            .map_err(|e| e.to_string())?;
        env.call_method(&stream, "write", "([B)V", &[JValue::Object(&buf)])
            .map_err(|e| e.to_string())?;
    }
    env.call_method(&stream, "flush", "()V", &[])
        .map_err(|e| e.to_string())?;
    env.call_method(&stream, "close", "()V", &[])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn export_subject_dirs(app: tauri::AppHandle, save_path: String) -> Result<usize, String> {
    let subject_dir = get_subject_dir(&app)?;
    if !subject_dir.exists() {
        return Err("subject 目录不存在".into());
    }

    // 1. 收集所有可导出的 db
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for dir_name in EXPORTABLE_DIRS {
        let dir = subject_dir.join(dir_name);
        if !dir.is_dir() {
            continue;
        }
        collect_db_files(&subject_dir, &dir, &mut files).map_err(|e| e.to_string())?;
    }

    if files.is_empty() {
        return Err("没有可导出的数据".into());
    }

    // 2. 安卓 SAF：用户选的位置是 content:// URI，不能直接 File::create
    //    先把 zip 打包到应用缓存目录，写完后再经 ContentResolver 拷到 URI
    #[allow(unused_mut)]
    let mut local_zip_path = save_path.clone();

    #[cfg(target_os = "android")]
    let mut content_uri: Option<String> = None;

    #[cfg(target_os = "android")]
    if save_path.starts_with("content://") {
        content_uri = Some(save_path.clone());
        let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        local_zip_path = cache.join("export_tmp.zip").to_string_lossy().into_owned();
    }

    // 3. 打包 zip 到 local_zip_path
    let file = fs::File::create(&local_zip_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for (rel, path) in &files {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        zip.start_file(rel, options).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;

    // 4. 安卓：把缓存里的 zip 拷到用户选的 content:// 位置
    #[cfg(target_os = "android")]
    if let Some(uri) = content_uri {
        copy_local_to_content_uri(&app, &uri, std::path::Path::new(&local_zip_path))?;
        let _ = fs::remove_file(&local_zip_path);
    }

    println!("[export] 已导出 {} 个文件到 {}", files.len(), local_zip_path);
    Ok(files.len())
}

// ============ 导入 ============

#[tauri::command]
async fn import_subject_zip(
    app: tauri::AppHandle,
    zip_path: String,
    import_state: tauri::State<'_, ImportState>,
) -> Result<serde_json::Value, String> {
    let subject_dir = get_subject_dir(&app)?;
    fs::create_dir_all(&subject_dir).map_err(|e| e.to_string())?;

    let cancel = import_state.cancel.clone();
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

    let mut duplicates: Vec<DuplicateInfo> = Vec::new();
    let mut imported = 0;
    let mut skipped = 0;

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

        // 只处理 .db
        if !name.ends_with(".db") {
            continue;
        }

        // 第一段目录名必须在白名单里
        let first_seg = name.split('/').next().unwrap_or("");
        if !EXPORTABLE_DIRS.contains(&first_seg) {
            skipped += 1;
            continue;
        }

        // 目标路径 = subject/<相对路径>
        let target_path = subject_dir.join(&name);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // 目标不存在：直接复制
        if !target_path.exists() {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            fs::write(&target_path, &buf).map_err(|e| e.to_string())?;
            imported += 1;
            continue;
        }

        // 目标存在：读取源 db，检查是否为空
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;

        // 写到临时文件，因为 rusqlite 只接受路径
        let tmp_path = tmp_root.join(format!("import_{}.db", uuid::Uuid::new_v4()));
        fs::write(&tmp_path, &buf).map_err(|e| e.to_string())?;

        let src_conn = match Connection::open(&tmp_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("打开源 db 失败: {}", e));
            }
        };

        // 源库没有 phrase_table
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
            skipped += 1;
            continue;
        }

        // 检查源库是否为空
        let count: i64 = src_conn
            .query_row("SELECT COUNT(*) FROM phrase_table", [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            // 源库为空，跳过，不覆盖目标
            drop(src_conn);
            let _ = fs::remove_file(&tmp_path);
            skipped += 1;
            continue;
        }

        // 源库有数据：打开目标库，逐条比对
        let dst_conn = Connection::open(&target_path).map_err(|e| e.to_string())?;
        dst_conn
            .execute(
                "CREATE TABLE IF NOT EXISTS phrase_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT, first_char TEXT, keyboard TEXT, list TEXT
            )",
                [],
            )
            .map_err(|e| e.to_string())?;

        // 加索引
        dst_conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_keyboard ON phrase_table(keyboard)",
                [],
            )
            .map_err(|e| e.to_string())?;

        // 开事务
        let tx = dst_conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;

        let mut stmt = src_conn
            .prepare("SELECT type, first_char, keyboard, list FROM phrase_table")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut cancelled = false;

        for row in rows {
            if cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let (t, fc, kb, list) = row.map_err(|e| e.to_string())?;

            // 查重：走索引，O(log N)
            let existing: Option<String> = tx
                .query_row(
                    "SELECT list FROM phrase_table WHERE keyboard = ?1",
                    params![kb],
                    |r| r.get(0),
                )
                .ok();

            match existing {
                Some(old_list) => {
                    duplicates.push(DuplicateInfo {
                        keyword: kb,
                        new_list: list,
                        old_list,
                        source_file: name.clone(),
                    });
                }
                None => {
                    tx.execute(
                        "INSERT INTO phrase_table (type, first_char, keyboard, list) VALUES (?1,?2,?3,?4)",
                        params![t, fc, kb, list],
                    )
                    .map_err(|e| e.to_string())?;
                    imported += 1;
                }
            }
        }

        // 先提交事务，再释放语句和连接
        tx.commit().map_err(|e| e.to_string())?;
        drop(stmt);
        drop(src_conn);
        let _ = fs::remove_file(&tmp_path);

        if cancelled {
            return Err("用户取消了导入".into());
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



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let my_id = uuid::Uuid::new_v4().to_string();
    let my_name = get_device_name();
    println!("[startup] device name = {}", my_name);

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let handle = app.handle().clone();

            let received = Arc::new(Mutex::new(Vec::new()));
            let devices = Arc::new(Mutex::new(HashMap::new()));
            let http_port = 51234u16;

            app.manage(AppState {
                devices: devices.clone(),
                my_id: my_id.clone(),
                my_name: my_name.clone(),
                http_port,
            });
            app.manage(ReceiveState {
                received: received.clone(),
            });
            app.manage(ScanState {
                running: Arc::new(AtomicBool::new(false)),
                generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            });
            app.manage(ImportState {
                cancel: Arc::new(AtomicBool::new(false)),
            });
            app.manage(SendState {
                cancel: Arc::new(AtomicBool::new(false)),
            });
            app.manage(MdnsState {
                daemon: Arc::new(Mutex::new(None)),
                service_fullname: Arc::new(Mutex::new(None)),
            });

            // 接收开关：默认关闭，只有打开接收面板时才允许收文件
            let receiving = Arc::new(AtomicBool::new(false));
            app.manage(ReceiveControl {
                enabled: receiving.clone(),
            });

            // 启动 HTTP 接收服务
            let save_dir = get_subject_dir(&handle)
                .unwrap_or_else(|_| PathBuf::from("./received"))
                .join("_received");
            std::fs::create_dir_all(&save_dir).ok();

            // 启动时清空上次会话遗留的接收缓存
            let received_root = save_dir.clone();
            if let Ok(entries) = std::fs::read_dir(&received_root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let _ = std::fs::remove_dir_all(&p);
                    }
                }
            }
            println!("[startup] 已清理 _received 缓存: {:?}", received_root);

            let h2 = handle.clone();
            let receiving_for_server = receiving.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    start_http_server(http_port, save_dir, received, receiving_for_server, h2).await
                {
                    eprintln!("HTTP 服务器启动失败: {}", e);
                }
            });

            // mDNS 广播由前端在打开发送/接收面板时调 start_broadcast 手动开启

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_cwd,
            read_cwd_file,
            write_cwd_file,
            save_phrase,
            load_all_phrases,
            delete_phrase,
            scan_subject_db_files,
            send_files_to_devices,
            get_received_files,
            import_received_files,
            cancel_import,
            resolve_duplicate_overwrite,
            start_scan,
            stop_scan,
            clear_received_files,
            cancel_send,
            start_broadcast,
            stop_broadcast,
            set_receiving,
            export_subject_dirs,
            import_subject_zip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
