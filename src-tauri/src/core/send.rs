// ============ 局域网发送 ============
// 负责：把本机 subject 下的 .db 通过 HTTP 发给选定设备，并在最后附带 manifest.json。

use reqwest;
use serde_json;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

use crate::core::device::AppState;
use crate::core::paths::{module_of_file, scan_subject_db_files};

/// 发送取消标志
pub struct SendState {
    pub cancel: Arc<AtomicBool>,
}

#[tauri::command]
pub async fn send_files_to_devices(
    app: AppHandle,
    state: State<'_, AppState>,
    send_state: State<'_, SendState>,
    device_ids: Vec<String>,
) -> Result<(), String> {
    let db_files = scan_subject_db_files(app.clone())?;
    if db_files.is_empty() {
        return Err("subject 目录下没有找到 .db 文件".into());
    }

    // 带上本机名，使接收端能显示「谁发的」（发现阶段 mDNS 展示的也是这个值）
    let my_name = state.my_name.clone();

    let devices = state.devices.lock().await;
    let targets: Vec<crate::core::device::DeviceInfo> = device_ids
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

    // 生成 manifest.json（与导出 export_subject_dirs 完全同构：{format, items:[{module, path}]}），
    // 接收端导入时按 manifest 的 module 派发到不同导入函数。path 取 subject 之后的相对路径，
    // 与接收端落盘的 relative_path 一致。
    let mut manifest_items: Vec<serde_json::Value> = Vec::new();
    for fp in &db_files {
        // 扁平结构：所有 .db 统一放在 english/ 下、以原始文件名区分
        // （english/phrase_db.db、english/note_db.db），与接收端 save_path 对齐，
        // 便于接收端 resolve_module 按 path 匹配并派发到对应导入函数
        let base = Path::new(fp)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown.db".to_string());
        let rel_str = format!("english/{}", base);
        let module = module_of_file(Path::new(fp))
            .unwrap_or("unknown")
            .to_string();
        manifest_items.push(serde_json::json!({ "module": module, "path": rel_str }));
    }
    let manifest = serde_json::json!({ "format": 1, "items": manifest_items });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;

    for device in &targets {
        for file_path in &db_files {
            if cancel.load(Ordering::SeqCst) {
                return Err("用户取消了发送".into());
            }
            let url = format!("http://{}:{}/upload", device.ip, device.port);
            let file_name = Path::new(file_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown.db".into());

            // 发送端按表名嗅探模块（phrase/note），随传输带上，
            // 使接收端导入时无需再猜，与导出 manifest 的 module 字段等价
            let module = module_of_file(Path::new(file_path))
                .unwrap_or("unknown")
                .to_string();

            // 短语库只发 source='user' 行（与导出 export_subject_dirs 同策略）：
            // base 是公共种子，不随个人数据互传；其它模块（笔记等）保持整库原样发送。
            // 过滤失败则回退整库，不阻断发送。
            let bytes = if module == "phrase" {
                let p = file_path.clone();
                match tokio::task::spawn_blocking(move || {
                    crate::english::phrase::export_user_rows_bytes(Path::new(&p))
                })
                .await
                {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => {
                        println!("[send] phrase 过滤失败，回退整库发送: {}", e);
                        tokio::fs::read(file_path)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                    Err(e) => {
                        println!("[send] phrase 过滤任务失败，回退整库发送: {}", e);
                        tokio::fs::read(file_path)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                }
            } else {
                tokio::fs::read(file_path)
                    .await
                    .map_err(|e| e.to_string())?
            };
            let part = reqwest::multipart::Part::bytes(bytes)
                .file_name(file_name.clone())
                .mime_str("application/octet-stream")
                .map_err(|e| e.to_string())?;
            let form = reqwest::multipart::Form::new()
                .text("relative_path", file_path.clone())
                .text("module", module)
                .text("from_device", my_name.clone())
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

        // 所有 db 文件发完后，再发送 manifest.json（file_name 固定为 manifest.json，
        // 接收端据此识别并落盘为本设备的 manifest，不入 received 列表）
        if cancel.load(Ordering::SeqCst) {
            return Err("用户取消了发送".into());
        }
        let mpart = reqwest::multipart::Part::bytes(manifest_bytes.clone())
            .file_name("manifest.json".to_string())
            .mime_str("application/octet-stream")
            .map_err(|e| e.to_string())?;
        let mform = reqwest::multipart::Form::new()
            .text("relative_path", format!("{}/manifest.json", device.name))
            .text("module", "manifest")
            .text("from_device", my_name.clone())
            .part("file", mpart);

        let mresp = client
            .post(&format!("http://{}:{}/upload", device.ip, device.port))
            .multipart(mform)
            .send()
            .await
            .map_err(|e| format!("发送清单到 {} 失败: {}", device.name, e))?;
        if !mresp.status().is_success() {
            return Err(format!(
                "发送清单到 {} 失败: HTTP {}",
                device.name,
                mresp.status()
            ));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn cancel_send(state: State<'_, SendState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}
