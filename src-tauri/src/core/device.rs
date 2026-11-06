// ============ 设备发现（mDNS）与设备名 ============
// 负责：本机设备名获取、局域网设备扫描、mDNS 广播开关，以及相关 State。

use local_ip_address;
use mdns_sd;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

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

pub struct ScanState {
    pub running: Arc<AtomicBool>,
    pub generation: Arc<AtomicU64>,
    /// 扫描用的 mDNS daemon 句柄（与广播 daemon 同款，都是后台 OS 线程）。
    /// ServiceDaemon 没有 Drop，必须显式 shutdown() 才会停线程，否则线程泄漏、进程关不掉。
    /// 这里存一份克隆，退出时由 ExitRequested 直接 shutdown，避免靠扫描循环自然退出带来的延迟与不确定性。
    pub daemon: Arc<Mutex<Option<mdns_sd::ServiceDaemon>>>,
}

/// mDNS 广播状态：把 daemon 存起来，方便 start/stop
pub struct MdnsState {
    pub daemon: Arc<Mutex<Option<mdns_sd::ServiceDaemon>>>,
    pub service_fullname: Arc<Mutex<Option<String>>>,
}

/// Android：通过 JNI 读 Settings.Global.DEVICE_NAME，失败退回 Build.MODEL
#[cfg(target_os = "android")]
pub fn get_device_name() -> String {
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
pub fn get_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "PhraseApp".into())
}

#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_state: State<'_, ScanState>,
) -> Result<(), String> {
    let my_gen = scan_state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    scan_state.running.store(true, Ordering::SeqCst);
    let running = scan_state.running.clone();
    let generation = scan_state.generation.clone();
    let devices = state.devices.clone();
    let my_id = state.my_id.clone();
    let app2 = app.clone();
    let scan_daemon = scan_state.daemon.clone();

    tauri::async_runtime::spawn(async move {
        let mdns = match mdns_sd::ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("mDNS daemon 创建失败: {}", e);
                return;
            }
        };

        // 存一份 daemon 克隆到 ScanState，退出时由 ExitRequested 直接 shutdown()，
        // 不必等扫描循环自然退出（ServiceDaemon 无 Drop，必须显式停线程，否则泄漏、进程关不掉）
        if let Ok(mut d) = scan_daemon.try_lock() {
            *d = Some(mdns.clone());
        }

        while running.load(Ordering::SeqCst) && generation.load(Ordering::SeqCst) == my_gen {
            devices.lock().await.clear();

            if let Ok(receiver) = mdns.browse("_phraseapp._tcp.local.") {
                let deadline = Instant::now() + Duration::from_millis(900);
                loop {
                    let timeout = deadline.saturating_duration_since(Instant::now());
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

            // 关闭前尽早退出：发完本轮设备列表后立刻再判一次 running，
            // 避免还要多睡 1s 才让循环退出（否则用户点关闭后要等整轮才释放 mDNS 线程）
            if !running.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        let _ = mdns.shutdown();
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_scan(scan_state: State<'_, ScanState>) -> Result<(), String> {
    scan_state.running.store(false, Ordering::SeqCst);
    Ok(())
}

/// 选择 mDNS 广播要注册的 IPv4 地址。
///
/// 背景：`local_ip()` 只从"持有默认路由的网卡"取 IP。电脑开着移动热点给手机连时，
/// 默认路由仍在路由器网卡上，取到的会是路由器网段 IP（如 192.168.1.x）——手机在
/// 热点网段（192.168.137.x）根本够不着；更糟的是 mdns-sd 应答查询时只回答与
/// "查询到达网卡"同网段的地址（`valid_ip_on_intf`），网段不符会静默不应答，
/// 导致手机侧"未发现设备"（电脑能发现手机、手机发现不了电脑，就是这条）。
///
/// 策略：枚举全部网卡，优先选 Windows 移动热点网段 192.168.137.0/24；
/// 没有热点网卡时退回 local_ip()（常规同网段场景，行为不变）。
fn pick_advertise_ipv4() -> String {
    if let Ok(ifas) = local_ip_address::list_afinet_netifas() {
        let hotspot_ip = ifas.iter().find_map(|(_, ip)| match ip {
            std::net::IpAddr::V4(a)
                if a.octets()[0] == 192 && a.octets()[1] == 168 && a.octets()[2] == 137 =>
            {
                Some(*a)
            }
            _ => None,
        });
        if let Some(ip) = hotspot_ip {
            return ip.to_string();
        }
    }
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn start_broadcast(
    state: State<'_, MdnsState>,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    if state.daemon.lock().await.is_some() {
        return Ok(());
    }

    // 显式取本机局域网 IP，Android 上 enable_addr_auto 不可靠
    let local_ip = pick_advertise_ipv4();
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
pub async fn stop_broadcast(state: State<'_, MdnsState>) -> Result<(), String> {
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
