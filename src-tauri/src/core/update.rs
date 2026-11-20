// ============ 冷更新 / 自动更新（后端全包）============
// 规格（用户拍板）：
//   - 启动时由前端 onMounted 异步调 check_update；本模块顺带把上次残留的 update/ 清理掉（异步）。
//   - update.json 记「上次检查日期」：桌面 exe 同级（便携式），手机 appdata 根（与 config.json 同级）。
//     当天已查 → 跳过；成功拿到 API 响应才写日期；写入失败静默（用户自己用管理员身份运行）。
//   - 版本：从 tauri.conf.json 的 version 读取并拼 v，与 GitCode tag_name 直接字符串比对，不一致即提示。
//   - 更新源：GitCode releases/latest API，按平台返回 exe / apk 的 attach 链接。
//   - 下载：reqwest 流式写到 update/（桌面 exe 同级、手机 appdata/update），经 event 推进度。
//   - 安装：下载完成自动拉起——桌面弹安装界面（/CLOSEAPPLICATIONS /NORESTARTAPPLICATIONS），
//     手机经 FileProvider 的 content:// URI 拉起系统安装器。
//   - 一切失败静默：返回 None / Ok(())，不阻断启动、不弹窗。

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;
#[cfg(target_os = "android")]
use tauri::Manager;

/// 当前版本号：从 tauri.conf.json（package.info）的 version 读取，拼上 v 前缀。
/// 配置里是 "0.2.9"，GitCode tag_name 是 "v0.2.9"，故统一成带 v 再比对。
/// 这样升级只需改 tauri.conf.json（+ Cargo.toml），不用再改这里。
fn current_version(app: &AppHandle) -> String {
    format!("v{}", app.package_info().version)
}

const UPDATE_JSON: &str = "update.json";
const UPDATE_DIR: &str = "update";

/// 返回给前端的更新信息
#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,   // 最新版本号（tag_name）
    pub url: String,       // 下载链接（按平台选好的 exe / apk）
    pub notes: String,     // 更新说明（release body）
    pub file_name: String, // 落盘文件名
}

/// 检查更新结果：一切判断在后端，前端只按 status 展示，不做任何版本比对。
/// - available：发现新版本，前端弹更新提示框（复用启动检查的弹窗）
/// - up_to_date：已是最新版本（仅手动检查返回；启动检查为免打扰走 silent）
/// - error：检查失败（仅手动检查返回，message 为提示文案）
/// - silent：静默，前端什么都不做（启动检查的所有非更新情况）
#[derive(Serialize)]
pub struct UpdateCheckResult {
    pub status: &'static str,
    pub info: Option<UpdateInfo>,
    pub message: String,
}

impl UpdateCheckResult {
    fn silent() -> Self {
        Self { status: "silent", info: None, message: String::new() }
    }
    fn up_to_date() -> Self {
        Self { status: "up_to_date", info: None, message: "当前版本为最新版本".into() }
    }
    fn error(msg: impl Into<String>) -> Self {
        Self { status: "error", info: None, message: msg.into() }
    }
    fn available(info: UpdateInfo) -> Self {
        Self { status: "available", info: Some(info), message: String::new() }
    }
}

/// 下载取消信号：watch 通道，false = 继续下载，true = 中断。
/// 用 watch 而非 AtomicBool：下载循环 select! 等待它，能在数据块到达之前立刻中断。
static DOWNLOAD_CANCEL: OnceLock<watch::Sender<bool>> = OnceLock::new();

fn cancel_tx() -> &'static watch::Sender<bool> {
    DOWNLOAD_CANCEL.get_or_init(|| watch::channel(false).0)
}

/// 中断正在进行的下载（前端「取消下载」按钮调用）。
/// 只改信号，不直接杀下载任务——下载循环收到信号后自行收尾并删除半成品文件。
#[tauri::command]
pub fn cancel_update() {
    let _ = cancel_tx().send(true);
}

/// 更新相关文件的根目录
/// - 桌面：exe 同级（update.json 与 update/ 都在这，便携式）
/// - 手机：app_data_dir 根（= app_config_dir，与 config.json 同级）
#[cfg(target_os = "android")]
fn update_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))
}

#[cfg(not(target_os = "android"))]
fn update_root(_app: &AppHandle) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取可执行文件路径: {}", e))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?;
    Ok(dir.to_path_buf())
}

/// 今天日期字符串 YYYY-MM-DD（chrono 本地时区）
fn today() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d").to_string()
}

/// 读取 update.json 里记录的检查日期
fn read_last_check(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("last_check")?.as_str().map(|s| s.to_string())
}

/// 写入检查日期（失败静默）
fn write_last_check(path: &Path) {
    let day = today();
    let json = serde_json::json!({ "last_check": day });
    if let Ok(content) = serde_json::to_string_pretty(&json) {
        let _ = fs::write(path, content);
    }
}

/// 异步清理上次残留的 update/ 目录（下载中断 / 用户取消安装留下的安装包）。
/// 放在 check_update 开头执行，不占 setup / 窗口渲染路径。
async fn cleanup_update_dir(root: &Path) {
    let dir = root.join(UPDATE_DIR);
    if dir.exists() {
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}

/// 检查更新。一切判断（版本比对 / 失败判定）在后端，前端只按 status 展示。
/// - 启动调用（force=false/None）：保持静默语义——无更新或任何失败都返回 silent，不打扰。
/// - 手动调用（force=true）：绕过日期记账，返回明确状态——有更新 / 已是最新 / 失败原因。
#[tauri::command]
pub async fn check_update(app: AppHandle, force: Option<bool>) -> Result<UpdateCheckResult, String> {
    let manual = force.unwrap_or(false);
    // 结果分流：手动检查要给出明确提示；启动检查一律静默
    let quiet = |r: UpdateCheckResult| -> UpdateCheckResult {
        if manual { r } else { UpdateCheckResult::silent() }
    };

    let root = match update_root(&app) {
        Ok(p) => p,
        Err(_) => return Ok(quiet(UpdateCheckResult::error("无法获取应用数据目录"))),
    };

    // 异步清残留（每次启动都清，保证 update/ 只放本次的新安装包）
    cleanup_update_dir(&root).await;

    let json_path = root.join(UPDATE_JSON);

    // 当天已查 → 跳过；手动点「检查更新」传 force=true 时绕过日期记账
    if !manual {
        if let Some(last) = read_last_check(&json_path) {
            if today() == last {
                return Ok(UpdateCheckResult::silent());
            }
        }
    }

    // 请求 GitCode API（超时 10s）
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Ok(quiet(UpdateCheckResult::error("检查失败：无法创建网络请求"))),
    };
    let resp = match client.get(crate::core::api::UPDATE_API).send().await {
        Ok(r) => r,
        Err(_) => return Ok(quiet(UpdateCheckResult::error("检查失败：网络请求失败，请检查网络"))),
    };
    if !resp.status().is_success() {
        return Ok(quiet(UpdateCheckResult::error(format!(
            "检查失败：服务器返回 {}",
            resp.status()
        ))));
    }
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Ok(quiet(UpdateCheckResult::error("检查失败：无法解析服务器响应"))),
    };

    // 成功拿到 API 响应 → 记今天日期（写失败静默）
    write_last_check(&json_path);

    // 版本比对：不一致就提示（含降级，用户明确要字符串比对）
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if tag.is_empty() {
        return Ok(quiet(UpdateCheckResult::error("检查失败：服务器未返回版本号")));
    }
    if tag == current_version(&app) {
        return Ok(quiet(UpdateCheckResult::up_to_date()));
    }

    // 按平台挑 attach 资源：桌面 PhraseNote.exe / 手机 PhraseNote.apk
    let wanted = if cfg!(target_os = "android") {
        "PhraseNote.apk"
    } else {
        "PhraseNote.exe"
    };
    let mut url = String::new();
    if let Some(assets) = body.get("assets").and_then(|v| v.as_array()) {
        for a in assets {
            let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = a.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if name == wanted && kind == "attach" {
                if let Some(u) = a.get("browser_download_url").and_then(|v| v.as_str()) {
                    url = u.to_string();
                    break;
                }
            }
        }
    }
    if url.is_empty() {
        return Ok(quiet(UpdateCheckResult::error(format!(
            "检查失败：未找到 {} 安装包",
            wanted
        ))));
    }

    let notes = body
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(UpdateCheckResult::available(UpdateInfo {
        version: tag,
        url,
        notes,
        file_name: wanted.to_string(),
    }))
}

/// 下载安装包到 update/，经 Tauri event 推进度，完成后自动拉起安装。
/// 事件名 `update-progress`，payload: { downloaded, total, percent }。
#[tauri::command]
pub async fn download_update(app: AppHandle, url: String, file_name: String) -> Result<(), String> {
    // 安全：文件名只允许固定白名单，拒绝路径穿越
    if file_name != "PhraseNote.exe" && file_name != "PhraseNote.apk" {
        return Err("非法文件名".into());
    }

    let root = update_root(&app)?;
    let dir = root.join(UPDATE_DIR);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let target = dir.join(&file_name);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt as _;

    // 文件写入放到 spawn_blocking 的独立线程：
    //   - std::fs 是阻塞 IO，直接跑在 async 上下文会卡住 tokio worker 线程；
    //   - tokio::fs::File 不是 Send，会让整个 command future 失去 Send（编译不过）。
    //
    // ★ 关键：数据通道的 sender 只能有主循环这一个持有者。
    //   写线程若也持有一份 sender，它阻塞在 recv() 上等消息，而 recv() 只有在
    //   所有 sender 都被 drop 后才返回 Err —— 而那份 sender 要等写线程自己返回才 drop，
    //   互相等待 → 永久死锁（表现为进度条停在 90% 多不动）。
    //   所以错误回报走独立通道（oneshot），不碰数据通道的 sender 计数。
    let (tx_file, rx_file) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);
    let target_for_writer = target.clone();
    let (tx_err, rx_err) = tokio::sync::oneshot::channel::<String>();
    let writer = tokio::task::spawn_blocking(move || {
        let mut file = match fs::File::create(&target_for_writer) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx_err.send(e.to_string());
                return;
            }
        };
        use std::io::Write as _;
        // recv 返回 Err = 所有 sender 已 drop = 主循环结束投递，正常收尾
        while let Ok(data) = rx_file.recv() {
            if let Err(e) = file.write_all(&data) {
                let _ = tx_err.send(e.to_string());
                return;
            }
        }
        // 收尾：刷盘，失败也回报
        if let Err(e) = file.flush() {
            let _ = tx_err.send(e.to_string());
        }
    });

    let mut downloaded: u64 = 0;
    // 新一轮下载开始前复位取消信号，并订阅一份接收端
    let tx = cancel_tx();
    let _ = tx.send_replace(false);
    let mut cancel_rx = tx.subscribe();

    // 进度事件节流：安卓上每次 emit 都要跨进程投递 JS 脚本到 WebView，
    // 管道容量仅 8；每块都发会把 UI 线程压死 → ANR → 系统杀进程（闪退）。
    // 这里按「百分比变化」且「距上次 ≥200ms」才发一次，全过程最多几十次。
    let mut last_emit_percent: u32 = u32::MAX;
    let mut last_emit_at = std::time::Instant::now();

    // 逐块下载：select! 让取消信号与数据块竞争，收到取消立即中断
    // wait_for 会先检查当前值，避免「取消发生在订阅之前」被吞掉
    let cancelled = loop {
        tokio::select! {
            // 用户点了「取消下载」→ 中断，删半成品，不拉起安装
            _ = cancel_rx.wait_for(|v| *v) => break true,
            chunk = stream.next() => {
                match chunk {
                    None => break false, // 流结束 = 下载完成
                    Some(chunk) => {
                        let chunk = chunk.map_err(|e| e.to_string())?;
                        // 交给阻塞写入线程落盘（通道满时会阻塞，等于背压）
                        // 发送失败 = 写线程已退出（磁盘错误等）
                        if tx_file.send(chunk.to_vec()).is_err() {
                            break false;
                        }
                        downloaded += chunk.len() as u64;
                        let percent = if total > 0 {
                            ((downloaded as f64 / total as f64) * 100.0) as u32
                        } else {
                            0
                        };
                        // 节流：百分比变了才发，且同一百分比至少间隔 200ms
                        let now = std::time::Instant::now();
                        if percent != last_emit_percent
                            && now.duration_since(last_emit_at) >= std::time::Duration::from_millis(200)
                        {
                            last_emit_percent = percent;
                            last_emit_at = now;
                            let _ = app.emit(
                                "update-progress",
                                serde_json::json!({ "downloaded": downloaded, "total": total, "percent": percent }),
                            );
                        }
                    }
                }
            }
        }
    };
    // 关闭写入通道 → 写线程 recv 返回 Err → flush 完退出；
    // 等待它结束，确保数据已完整落盘（否则装的是坏包）
    drop(tx_file);
    // 加超时兜底：万一写线程出现未预料的挂起，也不至于永久卡在 90% 多
    let write_done = tokio::time::timeout(std::time::Duration::from_secs(30), writer).await;
    if write_done.is_err() {
        // 刷盘超时：按失败处理，不拉起安装（宁可报错也不能装坏包）
        let _ = tokio::fs::remove_file(&target).await;
        return Err("写入安装包超时（落盘未完成）".into());
    }
    // 写线程回报的错误（磁盘满 / 权限 / flush 失败）
    let write_err = rx_err.await.ok();

    if cancelled {
        let _ = tokio::fs::remove_file(&target).await;
        return Ok(());
    }

    // 下载完成 → 拉起安装（两端实现分开）
    // 收尾前再查一次取消：避免「刚好在下载结束的瞬间」点了取消仍被拉起安装
    if *cancel_rx.borrow() {
        let _ = tokio::fs::remove_file(&target).await;
        return Ok(());
    }
    // 落盘失败（磁盘满 / 权限）→ 直接报错，不拉起安装（否则装的是坏包）
    if let Some(e) = write_err {
        return Err(format!("写入安装包失败: {}", e));
    }
    // 补发一次 100%：节流后最后一包可能因不足 200ms 间隔被跳过，进度条会卡在 99%
    let _ = app.emit(
        "update-progress",
        serde_json::json!({ "downloaded": downloaded, "total": total, "percent": 100 }),
    );
    launch_install(&app, &target)?;
    Ok(())
}

/// 拉起系统安装器安装已下载的 APK。
/// Android 8.0(API 26)+ 需先确认「允许安装未知来源应用」授权，否则启动安装器会静默失败。
#[cfg(target_os = "android")]
fn launch_install(_app: &AppHandle, apk: &Path) -> Result<(), String> {
    // ★ 不要用 app.run_on_main_thread()：
    //   它在安卓上只是把闭包异步派发到 tao event-loop 线程（tao 用 ndk::looper::ThreadLooper
    //   在独立线程跑事件循环，并不是 Android UI 线程），闭包内的错误也无法回传给调用方。
    //   这里直接在当前线程同步执行：Activity.startActivity() 允许从非 UI 线程调用
    //   （Android 内部会转到主线程执行），JNI 出错也能原样返回给前端显示。
    //
    // 安装包必须真实存在：FileProvider 对不存在的文件也能生成 content:// URI，
    // 但安装器拿到后解析失败，报错信息还很难懂。这里先挡一道，给个明白话。
    if !apk.exists() {
        return Err(format!("安装包不存在或已被清理: {}", apk.display()));
    }
    start_install_activity(apk)
}

/// 取出并格式化当前挂起的 Java 异常（类名 + 消息），然后清除它。
///
/// 为什么需要：jni-rs 的 `Error::JavaException` Display 只有一句
/// "Java exception was thrown"——真正的异常（ClassNotFoundException /
/// IllegalArgumentException / ActivityNotFoundException…）全在 JNI 的
/// pending exception 里。原来的代码直接 clear 掉，前端只看到那句废话，
/// 完全没法定位是哪一步、什么原因。标准处理顺序：
///   1. exception_describe()  把堆栈打到 logcat（adb logcat 可看）
///   2. exception_occurred()  取出 Throwable 局部引用（清除后该引用仍有效）
///   3. exception_clear()     清除异常——之后调 JNI 方法才是安全的
///      （带着 pending exception 调 JNI 是未定义行为，ART 可能直接 abort）
///   4. Throwable.toString()  拿到 "java.lang.XxxException: message"
#[cfg(target_os = "android")]
fn take_pending_exception(env: &mut jni::JNIEnv) -> Option<String> {
    if !env.exception_check().unwrap_or(false) {
        return None;
    }
    let _ = env.exception_describe();
    let throwable = env.exception_occurred().ok()?;
    let _ = env.exception_clear();
    if throwable.as_raw().is_null() {
        return Some("未知 Java 异常（取不到异常对象）".into());
    }
    // Throwable.toString() → "java.lang.IllegalArgumentException: xxx"
    // 注意：get_string 返回的 JavaStr 借用 JString，必须当场转成 owned String，
    // 不能把借用传出闭包。
    let mut msg = env
        .call_method(&throwable, "toString", "()Ljava/lang/String;", &[])
        .ok()
        .and_then(|v| v.l().ok())
        .and_then(|s| {
            env.get_string((&s).into())
                .map(|v| v.to_string_lossy().into_owned())
                .ok()
        });
    if msg.as_deref().map(str::trim).unwrap_or("").is_empty() {
        msg = env
            .call_method(&throwable, "getMessage", "()Ljava/lang/String;", &[])
            .ok()
            .and_then(|v| v.l().ok())
            .and_then(|s| {
                env.get_string((&s).into())
                    .map(|v| v.to_string_lossy().into_owned())
                    .ok()
            });
    }
    Some(
        msg.filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "未知 Java 异常".into()),
    )
}

/// 在当前（已 attach 的）JNI 线程上执行安装器拉起。
#[cfg(target_os = "android")]
fn start_install_activity(apk: &Path) -> Result<(), String> {
    use jni::objects::{JObject, JValue};

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut guard = vm
        .attach_current_thread()
        .map_err(|e| e.to_string())?;
    let env = &mut *guard;
    // ★ 注意：ndk_context 里存的是 **Application**（tao ndk_glue.rs 传的是
    //   activity.getApplicationContext()），不是 Activity。本应用 <application> 没写
    //   android:name，所以它是框架类 android.app.Application。所有 Context 方法
    //   （getPackageName/getPackageManager/getClassLoader/startActivity）在它上面都能用。
    let app_ctx = unsafe { JObject::from_raw(ctx.context().cast()) };

    // 任何一步失败都清掉挂起的 JNI 异常（否则后续 JNI 调用在「异常待处理」状态下
    // 属于未定义行为，ART 会直接 abort 进程，release 下 panic=abort → 秒退无日志），
    // 并把「步骤名 + 真实 Java 异常文本」一起返回给前端——原来只有一句
    // "Java exception was thrown"，既不知道哪一步挂的、也不知道为什么。
    macro_rules! jni_try {
        ($step:literal, $e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => {
                    let detail = take_pending_exception(env).unwrap_or_else(|| e.to_string());
                    return Err(format!("{}: {}", $step, detail));
                }
            }
        };
    }

    eprintln!("[update] 拉起安装器，APK: {}", apk.display());

    // File(apkPath)
    let path_str = jni_try!("构造 APK 路径字符串", env.new_string(apk.to_string_lossy().as_ref()));
    let file_cls = jni_try!("加载 java/io/File", env.find_class("java/io/File"));
    let file_obj = jni_try!(
        "构造 File 对象",
        env.new_object(
            file_cls,
            "(Ljava/lang/String;)V",
            &[JValue::Object(&path_str)]
        )
    );

    // authority = applicationId + ".fileprovider"
    // activity.getPackageName() → String（分步取值，照搬 saf.rs 的写法避免临时引用问题）
    let pkg_obj = jni_try!(
        "获取包名",
        env.call_method(&app_ctx, "getPackageName", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
    );
    let pkg_name: String = jni_try!(
        "读取包名",
        env.get_string((&pkg_obj).into())
            .map(|s| s.to_string_lossy().into_owned())
    );
    let authority = format!("{}.fileprovider", pkg_name);
    let auth_str = jni_try!("构造 authority", env.new_string(&authority));
    // Intent 类提前取好：授权检查分支和正常安装分支都要用
    let intent_cls = jni_try!("加载 android/content/Intent", env.find_class("android/content/Intent"));
    eprintln!("[update] FileProvider authority: {}", authority);

    // ★ 关键：不能用 find_class("androidx/core/content/FileProvider")
    //   find_class 走 FindClass，在「自己创建并 attach 的原生线程」上只有系统类加载器，
    //   找不到应用自己的 androidx.* 类 → ClassNotFoundException。
    //   正确做法：拿「应用自己的类加载器」再 loadClass。
    //
    //   ★★ 大坑（2026-11-20 真机实测踩到，勿回退）★★
    //   ndk_context 里存的 context 是 **Application**，不是 Activity——
    //   tao 的 ndk_glue.rs 里 `ndk_context::initialize_android_context(vm, application)`
    //   传的就是 activity.getApplicationContext()。
    //   而本应用 manifest 的 <application> 没有 android:name → Application 是框架类
    //   android.app.Application → getClass().getClassLoader() 拿到的是 **BootClassLoader**
    //   （只能看框架类，看不到应用 dex）→ loadClass("androidx.core.content.FileProvider")
    //   必然 ClassNotFoundException（实测异常信息：java.lang.ClassNotFoundException:
    //   androidx.core.content.FileProvider）。
    //   正解：Context.getClassLoader() —— 返回该 Context 所属应用的 PathClassLoader
    //   （含应用全部 dex），loadClass 才能找到 androidx.*。
    let loader = jni_try!(
        "获取应用 ClassLoader（Context.getClassLoader）",
        env.call_method(&app_ctx, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .and_then(|v| v.l())
    );
    // ClassLoader.loadClass("androidx.core.content.FileProvider") → Class
    let fp_name = jni_try!(
        "构造 FileProvider 类名",
        env.new_string("androidx.core.content.FileProvider")
    );
    let fp_cls_obj = jni_try!(
        "加载 androidx.core.content.FileProvider（失败说明 APK 里没有这个类）",
        env.call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&fp_name)],
        )
        .and_then(|v| v.l())
    );
    let fp_cls = unsafe { jni::objects::JClass::from_raw(fp_cls_obj.as_raw()) };

    // FileProvider.getUriForFile(context, authority, file) → content:// URI
    //   常见异常：IllegalArgumentException "Failed to find configured root that contains …"
    //   ——即 res/xml/file_paths.xml 没覆盖到安装包所在目录。
    let uri_obj = jni_try!(
        "FileProvider.getUriForFile（若提示 Failed to find configured root，检查 res/xml/file_paths.xml）",
        env.call_static_method(
            &fp_cls,
            "getUriForFile",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/io/File;)Landroid/net/Uri;",
            &[
                JValue::Object(&app_ctx),
                JValue::Object(&auth_str),
                JValue::Object(&file_obj),
            ],
        )
        .and_then(|v| v.l())
    );
    eprintln!("[update] content:// URI 生成成功");

    // ── Android 8.0(API 26)+：先确认「允许安装未知来源应用」授权 ──
    //   未授权时直接启动安装器会静默失败（看不到任何界面）。
    //   这里用 PackageManager.canRequestPackageInstalls() 检查，没授权就跳系统设置页，
    //   用户授权后再点一次「更新」即可。
    let pm_obj = jni_try!(
        "获取 PackageManager",
        env.call_method(&app_ctx, "getPackageManager", "()Landroid/content/pm/PackageManager;", &[])
            .and_then(|v| v.l())
    );
    // canRequestPackageInstalls() 是 API 26+ 才有的方法；低版本抛 NoSuchMethodError，
    // 这里用异常是否挂起来判断版本，低版本直接当作「无需授权」。
    let can_install: bool = match env.call_method(
        &pm_obj,
        "canRequestPackageInstalls",
        "()Z",
        &[],
    ) {
        Ok(v) => v.z().unwrap_or(true),
        Err(_) => {
            // 低版本无此方法：清异常并视为无需授权
            let _ = clear_pending_exception(env);
            true
        }
    };
    if !can_install {
        // 跳到「未知来源应用」设置页：Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES
        // Intent(action, Uri.parse("package:" + pkgName))
        let action_manage = jni_try!(
            "构造设置页 action",
            env.new_string("android.settings.action.MANAGE_UNKNOWN_APP_SOURCES")
        );
        let pkg_uri_str = jni_try!(
            "构造 package: URI 字符串",
            env.new_string(&format!("package:{}", pkg_name))
        );
        let uri_cls = jni_try!("加载 android/net/Uri", env.find_class("android/net/Uri"));
        let pkg_uri = jni_try!(
            "Uri.parse",
            env.call_static_method(
                uri_cls,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&pkg_uri_str)],
            )
            .and_then(|v| v.l())
        );
        let settings_intent = jni_try!(
            "构造设置页 Intent",
            env.new_object(
                intent_cls,
                "(Ljava/lang/String;)V",
                &[JValue::Object(&action_manage)],
            )
        );
        let settings_intent = jni_try!(
            "设置页 Intent setData",
            env.call_method(
                &settings_intent,
                "setData",
                "(Landroid/net/Uri;)Landroid/content/Intent;",
                &[JValue::Object(&pkg_uri)],
            )
            .and_then(|v| v.l())
        );
        let settings_intent = jni_try!(
            "设置页 Intent addFlags",
            env.call_method(
                &settings_intent,
                "addFlags",
                "(I)Landroid/content/Intent;",
                &[JValue::Int(0x10000000)], // FLAG_ACTIVITY_NEW_TASK
            )
            .and_then(|v| v.l())
        );
        jni_try!(
            "跳转系统设置页",
            env.call_method(
                &app_ctx,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[JValue::Object(&settings_intent)]
            )
        );
        return Err("请先在系统设置中允许本应用安装未知来源应用，然后重新点击「更新」".into());
    }

    // ── 拉起安装器：先 ACTION_INSTALL_PACKAGE，失败降级 ACTION_VIEW ──
    //   INSTALL_PACKAGE 是 API 26+ 官方推荐；少数机型/ROM 上没有 Activity 认这个
    //   action（抛 ActivityNotFoundException），这时退回 ACTION_VIEW（旧机型通用）。
    match fire_install_intent(
        env,
        &app_ctx,
        &intent_cls,
        &uri_obj,
        "android.intent.action.INSTALL_PACKAGE",
    ) {
        Ok(()) => {
            eprintln!("[update] 已拉起系统安装器（ACTION_INSTALL_PACKAGE）");
            Ok(())
        }
        Err(e1) => {
            eprintln!("[update] ACTION_INSTALL_PACKAGE 拉起失败：{}", e1);
            match fire_install_intent(
                env,
                &app_ctx,
                &intent_cls,
                &uri_obj,
                "android.intent.action.VIEW",
            ) {
                Ok(()) => {
                    eprintln!("[update] 已降级用 ACTION_VIEW 拉起系统安装器");
                    Ok(())
                }
                Err(e2) => Err(format!(
                    "拉起系统安装器失败：{}（降级 ACTION_VIEW 也失败：{}）",
                    e1, e2
                )),
            }
        }
    }
}

/// 用指定 action 构造安装 Intent 并 startActivity。
/// 每一步失败都带「步骤名 + 真实 Java 异常文本」返回，方便定位。
#[cfg(target_os = "android")]
fn fire_install_intent(
    env: &mut jni::JNIEnv,
    app_ctx: &jni::objects::JObject,
    intent_cls: &jni::objects::JClass,
    uri_obj: &jni::objects::JObject,
    action: &str,
) -> Result<(), String> {
    use jni::objects::JValue;

    macro_rules! jni_try {
        ($step:literal, $e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => {
                    let detail = take_pending_exception(env).unwrap_or_else(|| e.to_string());
                    return Err(format!("{}: {}", $step, detail));
                }
            }
        };
    }

    let action_str = jni_try!("构造 Intent action", env.new_string(action));
    let intent = jni_try!(
        "构造 Intent",
        env.new_object(intent_cls, "(Ljava/lang/String;)V", &[JValue::Object(&action_str)])
    );

    // setDataAndType(uri, "application/vnd.android.package-archive")
    let mime = jni_try!(
        "构造 MIME",
        env.new_string("application/vnd.android.package-archive")
    );
    let intent = jni_try!(
        "setDataAndType",
        env.call_method(
            &intent,
            "setDataAndType",
            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(uri_obj), JValue::Object(&mime)],
        )
        .and_then(|v| v.l())
    );

    // addFlags(FLAG_GRANT_READ_URI_PERMISSION | FLAG_ACTIVITY_NEW_TASK)
    let flags: jni::sys::jint = 0x00000001 | 0x10000000;
    let intent = jni_try!(
        "addFlags",
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(flags)],
        )
        .and_then(|v| v.l())
    );

    jni_try!(
        "startActivity",
        env.call_method(
            app_ctx,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)]
        )
    );
    Ok(())
}

/// 清除挂起的 JNI 异常（避免异常状态下继续调用 JNI 导致进程 abort）
#[cfg(target_os = "android")]
fn clear_pending_exception(env: &mut jni::JNIEnv) -> Result<(), String> {
    match env.exception_check() {
        Ok(true) => {
            let _ = env.exception_clear();
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(not(target_os = "android"))]
fn launch_install(_app: &AppHandle, installer: &Path) -> Result<(), String> {
    // 弹安装界面（非静默），关掉运行中的旧 exe（删 A）、避免装完双开（保留 B 的 postinstall 拉起）
    std::process::Command::new(installer)
        .args([
            "/CLOSEAPPLICATIONS",
            "/NORESTARTAPPLICATIONS",
        ])
        .spawn()
        .map_err(|e| format!("启动安装程序失败: {}", e))?;
    Ok(())
}
