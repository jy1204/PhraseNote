// ============ 接收 HTTP 服务 / 上传落盘 / 接收开关 ============
// 负责：在 51234 端口起 axum 接收服务，把对端发来的文件落盘到 _received/{device}/，
// 维护接收列表与接收开关。

use axum::{
    extract::Multipart,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{Mutex, Notify};

use crate::core::paths::sanitize_device;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedFile {
    pub id: String,
    /// 发送端设备名（展示用，如「我的手机」）
    pub from_device: String,
    /// 设备目录名（由 from_device 清洗得到，用于把同一设备所有文件放进同一临时文件夹）
    pub device: String,
    /// 相对 subject 的路径（如 english/我的笔记/note_db.db），与发送端 manifest 的 path 对齐，
    /// 供导入时按 manifest 查 module 派发到对应导入函数
    pub relative_path: String,
    pub saved_path: String,
    /// 模块类型：phrase / note / unknown。manifest 缺失或查不到时作为回退，
    /// 正常情况下导入以 manifest.json 为准（与导出 manifest 的 module 字段等价）
    pub module: String,
}

pub struct ReceiveState {
    pub received: Arc<Mutex<Vec<ReceivedFile>>>,
}

/// 接收开关：只有接收面板打开时才允许 handle_upload 接受文件
pub struct ReceiveControl {
    pub enabled: Arc<AtomicBool>,
}

/// HTTP 服务任务句柄：启动后保存，应用退出时用于 abort / 触发优雅关闭，以释放 51234 端口
pub struct ServerHandle {
    pub task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 优雅关闭信号：退出时 notify，让 axum 主动断开所有连接（含卡死在半开连接上的上传任务），
    /// 避免这些连接处理任务拖住 tokio 运行时关闭、进程关不掉。
    pub shutdown: Arc<Notify>,
}

#[allow(dead_code)]
#[derive(Clone)]
struct HttpState {
    save_dir: PathBuf,
    received: Arc<Mutex<Vec<ReceivedFile>>>,
    app: AppHandle,
    receiving: Arc<AtomicBool>,
}

pub async fn start_http_server(
    port: u16,
    save_dir: PathBuf,
    received: Arc<Mutex<Vec<ReceivedFile>>>,
    receiving: Arc<AtomicBool>,
    app: AppHandle,
    server_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    shutdown: Arc<Notify>,
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

    // 用 TcpSocket 显式打开 SO_REUSEADDR：避免上一次进程退出后端口处于 TIME_WAIT 时，
    // 立刻重启报 "Address already in use"（端口占用残留的根因之一）
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let socket = tokio::net::TcpSocket::new_v4().map_err(|e| e.to_string())?;
    socket.set_reuseaddr(true).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    socket.set_reuseport(true).ok();
    socket.bind(addr).map_err(|e| e.to_string())?;
    let listener = socket.listen(1024).map_err(|e| e.to_string())?;

    let handle = tokio::spawn(async move {
        // 优雅关闭：收到 shutdown 信号后 axum 停止接收新连接并主动断开现有连接，
        // 让卡在半开连接上的上传处理任务立即报错结束，避免拖住进程退出。
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            shutdown.notified().await;
        });
        server.await.ok();
    });
    *server_handle.lock().await = Some(handle);

    Ok(())
}

async fn handle_upload(
    axum::extract::State(state): axum::extract::State<HttpState>,
    multipart: Multipart,
) -> Result<String, String> {
    // 只有接收面板打开时才接受文件
    if !state.receiving.load(Ordering::SeqCst) {
        return Err("对方未打开接收面板".into());
    }

    // 网络切换 / 对端掉线时，半开 TCP 连接不会立即 RST，read 会一直阻塞
    //（TCP 默认 keepalive 探测可达数小时）。若不加超时，该连接的处理任务永不结束，
    // 会拖住 tokio 运行时关闭，导致进程无法退出。局域网传小 db 通常 <1s，30s 足以判定连接已死。
    match tokio::time::timeout(Duration::from_secs(30), handle_upload_inner(state, multipart)).await {
        Ok(res) => res,
        Err(_) => Err("接收超时，连接可能已断开".into()),
    }
}

async fn handle_upload_inner(state: HttpState, mut multipart: Multipart) -> Result<String, String> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::new();
    let mut module = String::new();
    let mut from_device = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "module" => {
                module = field.text().await.map_err(|e| e.to_string())?;
            }
            "from_device" => {
                from_device = field.text().await.map_err(|e| e.to_string())?;
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

    // 旧版本发送端不带 from_device 时回退为「未知设备」
    if from_device.is_empty() {
        from_device = "未知设备".to_string();
    }

    // 同一设备所有文件放进同一个临时文件夹（清洗后的设备名），便于按设备统一导入/清理
    let device = sanitize_device(&from_device);

    // manifest.json：接收端据此按 module 派发导入函数。
    // 单独保存为本设备目录下的 manifest.json，且不进入 received 列表（前端不展示、不导入）。
    if file_name == "manifest.json" {
        let manifest_path = state.save_dir.join(&device).join("manifest.json");
        if let Some(parent) = manifest_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        tokio::fs::write(&manifest_path, &bytes)
            .await
            .map_err(|e| e.to_string())?;
        return Ok("OK".into());
    }

    // 扁平落盘：同一设备所有 .db 统一放在 english/ 下、以原始文件名区分
    // （english/phrase_db.db、english/note_db.db），与发送端 manifest 的 path 对齐，
    // 便于 resolve_module 按 path 匹配并派发导入函数。
    let rel_str = format!("english/{}", file_name);
    let target_dir = state.save_dir.join(&device);
    let save_path = target_dir.join(&rel_str);
    // 建好嵌套父目录（english/ 这一层）
    if let Some(parent) = save_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::write(&save_path, &bytes)
        .await
        .map_err(|e| e.to_string())?;

    let mut received = state.received.lock().await;

    // 幂等去重：同一设备（device）+ 同一相对路径（relative_path，即同文件名）只保留一条记录。
    // 对端重复发送相同的 .db 时，磁盘落盘路径相同会互相覆盖（本就幂等），
    // 但旧实现每次都生成新 id 并 push，导致 received 列表凭空叠加出重复条目
    //（「第一次 2 个、第二次又发同样的变 4 个」的根因）。这里改为复用已有条目。
    let received_file = if let Some(existing) =
        received.iter_mut().find(|f| f.device == device && f.relative_path == rel_str)
    {
        // 命中：复用旧 id，仅刷新展示/导入所需的元信息（展示名可能变化、module 以最新为准）
        existing.from_device = from_device.clone();
        existing.saved_path = save_path.to_string_lossy().into_owned();
        existing.module = module.clone();
        existing.clone()
    } else {
        let rf = ReceivedFile {
            id: file_id,
            from_device: from_device.clone(),
            device: device.clone(),
            relative_path: rel_str,
            saved_path: save_path.to_string_lossy().into_owned(),
            module,
        };
        received.push(rf.clone());
        rf
    };

    drop(received);
    let _ = state.app.emit("file_received", &received_file);
    Ok("OK".into())
}

#[tauri::command]
pub async fn get_received_files(
    state: State<'_, ReceiveState>,
) -> Result<Vec<ReceivedFile>, String> {
    let files = state.received.lock().await;
    Ok(files.clone())
}

#[tauri::command]
pub async fn set_receiving(
    state: State<'_, ReceiveControl>,
    enabled: bool,
) -> Result<(), String> {
    state.enabled.store(enabled, Ordering::SeqCst);
    println!("[receive] set_receiving = {}", enabled);
    Ok(())
}
