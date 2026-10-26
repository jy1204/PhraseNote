// ============ 应用配置（持久化到 config.json）============
// 负责：读取 / 保存外观偏好
//   theme  —— 外观主题（dark / light / system）
//   scheme —— 主题配色（"" | "blue" | "sakura"）；空串视为默认「科技蓝」
// config.json 位置：桌面端与可执行文件同目录（便携式）；Android 为应用私有配置目录。文件不存在时：
//   主题按「跟随系统」、配色按「科技蓝」兜底（两者均由前端处理 None）。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Serialize, Deserialize, Default, Clone)]
struct AppConfig {
    #[serde(default)]
    theme: String, // "dark" | "light" | "system"
    #[serde(default)]
    scheme: String, // "" | "blue" | "sakura"
}

/// 配置文件所在目录
/// - 桌面端（Windows 等）：与可执行文件同目录（便携式，配置随软件走）
/// - Android：应用私有配置目录（安装目录只读，无法写 exe 同目录）
#[cfg(target_os = "android")]
fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("无法获取应用配置目录: {}", e))
}

#[cfg(not(target_os = "android"))]
fn config_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("无法获取可执行文件路径: {}", e))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?;
    Ok(dir.to_path_buf())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(CONFIG_FILE_NAME))
}

/// 读取现有配置；文件不存在或解析失败都返回默认值
/// （前端据此走「跟随系统 + 默认配色」）
fn read_config(app: &AppHandle) -> Result<AppConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

/// 写回配置。采用「读–改–写」而不是整结构覆盖：
/// 这样保存主题时不会把已保存的配色抹掉，反之亦然。
fn write_config(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let dir = config_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(config_path(app)?, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// 读取外观主题：空值 → 返回 None（前端按「跟随系统」处理）
#[tauri::command]
pub fn load_app_config(app: AppHandle) -> Result<Option<String>, String> {
    let cfg = read_config(&app)?;
    if cfg.theme.is_empty() {
        Ok(None)
    } else {
        Ok(Some(cfg.theme))
    }
}

/// 保存外观主题（theme 取值：dark / light / system），其余配置项保留原值
#[tauri::command]
pub fn save_app_config(app: AppHandle, theme: String) -> Result<(), String> {
    let mut cfg = read_config(&app)?;
    cfg.theme = theme;
    write_config(&app, &cfg)
}

/// 读取主题配色：空值 → 返回 None（前端按「科技蓝」处理）
#[tauri::command]
pub fn load_app_scheme(app: AppHandle) -> Result<Option<String>, String> {
    let cfg = read_config(&app)?;
    if cfg.scheme.is_empty() {
        Ok(None)
    } else {
        Ok(Some(cfg.scheme))
    }
}

/// 保存主题配色（scheme 取值：blue / sakura），其余配置项保留原值
#[tauri::command]
pub fn save_app_scheme(app: AppHandle, scheme: String) -> Result<(), String> {
    let mut cfg = read_config(&app)?;
    cfg.scheme = scheme;
    write_config(&app, &cfg)
}
