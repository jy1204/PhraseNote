mod english {
    pub mod phrase;
    pub mod note;
}
mod core;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::Manager;
use tokio::sync::Mutex;

use crate::core::device::{get_device_name, AppState, MdnsState, ScanState};
use crate::core::import::ImportState;
use crate::core::paths::get_subject_dir;
use crate::core::receive::{ReceiveControl, ReceiveState, ServerHandle, start_http_server};
use crate::core::send::SendState;

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
                generation: Arc::new(AtomicU64::new(0)),
                daemon: Arc::new(Mutex::new(None)),
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

            // HTTP 服务任务句柄：启动后写入，退出时用于 abort / 触发优雅关闭以释放 51234 端口
            let http_server_handle = Arc::new(Mutex::new(None::<tokio::task::JoinHandle<()>>));
            let http_shutdown = Arc::new(tokio::sync::Notify::new());
            app.manage(ServerHandle {
                task: http_server_handle.clone(),
                shutdown: http_shutdown.clone(),
            });

            // 接收开关：默认关闭，只有打开接收面板时才允许收文件
            let receiving = Arc::new(AtomicBool::new(false));
            app.manage(ReceiveControl {
                enabled: receiving.clone(),
            });

            // 首次启动环境初始化 + 基础词迁移：已从 .setup 移至前端启动时调用
            // （ensure_init 命令），以便窗口先渲染、前端先弹全局遮罩，再异步跑迁移，
            // 避免启动期白屏。此处不再阻塞。

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
            let sh = http_server_handle.clone();
            let sd = http_shutdown.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_http_server(
                    http_port,
                    save_dir,
                    received,
                    receiving_for_server,
                    h2,
                    sh,
                    sd,
                )
                .await
                {
                    eprintln!("HTTP 服务器启动失败: {}", e);
                }
            });

            // mDNS 广播由前端在打开发送/接收面板时调 start_broadcast 手动开启

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::english::phrase::save_phrase,
            crate::english::phrase::load_all_phrases,
            crate::english::phrase::load_phrases_page,
            crate::english::phrase::search_phrases,
            crate::english::phrase::keyword_exists,
            crate::english::phrase::append_phrase,
            crate::english::phrase::delete_phrase,
            crate::core::paths::scan_subject_db_files,
            crate::core::send::send_files_to_devices,
            crate::core::receive::get_received_files,
            crate::core::import::import_received_files,
            crate::core::import::cancel_import,
            crate::english::phrase::resolve_duplicate_overwrite,
            crate::english::phrase::resolve_duplicates_batch,
            crate::english::phrase::batch_entry_phrases,
            crate::core::device::start_scan,
            crate::core::device::stop_scan,
            crate::core::import::clear_received_files,
            crate::core::import::clear_received_files_by_ids,
            crate::core::send::cancel_send,
            crate::core::device::start_broadcast,
            crate::core::device::stop_broadcast,
            crate::core::receive::set_receiving,
            crate::core::export::export_subject_dirs,
            crate::core::import::import_subject_zip,
            crate::english::note::load_note_tree,
            crate::english::note::load_note_content,
            crate::english::note::create_folder,
            crate::english::note::create_note,
            crate::english::note::save_folder,
            crate::english::note::save_note,
            crate::english::note::delete_folder,
            crate::english::note::delete_note,
            crate::english::note::move_note,
            crate::english::note::batch_move_notes,
            crate::english::note::batch_delete_notes,
            crate::english::note::batch_delete_folders,
            crate::english::note::update_note_content,
            crate::core::config::load_app_config,
            crate::core::config::save_app_config,
            crate::core::config::load_app_scheme,
            crate::core::config::save_app_scheme,
            crate::core::init::ensure_init
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 应用退出时优雅释放资源：停掉 mDNS 广播、中止 HTTP 服务任务，
            // 避免 51234 端口 / mDNS 套接字残留（这正是「软件关闭后端口占用」的根因）
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(mdns_state) = app_handle.try_state::<MdnsState>() {
                    if let Ok(mut daemon_slot) = mdns_state.daemon.try_lock() {
                        if let Some(mdns) = daemon_slot.take() {
                            if let Ok(mut fn_slot) = mdns_state.service_fullname.try_lock() {
                                if let Some(fullname) = fn_slot.take() {
                                    let _ = mdns.unregister(&fullname);
                                }
                            }
                            let _ = mdns.shutdown();
                        }
                    }
                }
                if let Some(server) = app_handle.try_state::<ServerHandle>() {
                    // 先触发优雅关闭：主动断开所有连接，让卡在半开连接上的上传任务立即报错结束，
                    // 避免它们拖住 tokio 运行时关闭、进程关不掉（网络切换掉线时的典型卡死场景）。
                    server.shutdown.notify_one();
                    if let Ok(mut guard) = server.task.try_lock() {
                        if let Some(handle) = guard.take() {
                            handle.abort();
                        }
                    }
                }
                // 关键：扫描任务内部持有 mdns_sd 的后台轮询线程，而 ServiceDaemon 没有 Drop，
                // 必须显式 shutdown() 才会停线程，否则线程泄漏、进程关不掉。
                // 这里直接对本设备扫描 daemon 调 shutdown()（与广播 daemon 同策略），
                // 同时把 running 置 false 让循环自然退出做双保险。
                if let Some(scan_state) = app_handle.try_state::<ScanState>() {
                    scan_state.running.store(false, Ordering::SeqCst);
                    if let Ok(mut d) = scan_state.daemon.try_lock() {
                        if let Some(mdns) = d.take() {
                            let _ = mdns.shutdown();
                        }
                    }
                }
            }
        });
}
