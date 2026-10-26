// ============ 路径 / subject 目录工具 ============
// 负责：subject 根目录求解、db 文件扫描/收集、模块嗅探、设备名清洗等共享基础设施。

use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
#[cfg(mobile)]
use tauri::Manager;

/// 可导出/可导入的白名单根目录（当前仅 english）
pub const EXPORTABLE_DIRS: &[&str] = &["english"];

#[cfg(desktop)]
pub fn get_subject_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;
    Ok(exe_dir.join("data").join("subject"))
}

#[cfg(mobile)]
pub fn get_subject_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(data_dir.join("subject"))
}

#[tauri::command]
pub fn scan_subject_db_files(app: AppHandle) -> Result<Vec<String>, String> {
    let root = get_subject_dir(&app)?;
    let mut result = Vec::new();
    if !root.exists() {
        return Ok(result);
    }
    // 仅扫描白名单目录（与导出 export_subject_dirs 保持一致），
    // 不再无差别递归整个 subject/，避免把白名单外的科目也发出去
    for dir_name in EXPORTABLE_DIRS {
        let dir = root.join(dir_name);
        if !dir.is_dir() {
            continue;
        }
        walk_dir(&dir, &mut result).map_err(|e| e.to_string())?;
    }
    Ok(result)
}

fn walk_dir(dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
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

/// 递归收集某个目录下所有 .db 文件的相对路径（相对 subject 根）
pub fn collect_db_files(
    base: &Path,
    dir: &Path,
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

// 嗅探 db 文件属于哪个模块（按表名判断），用于导出/导入时生成或解析 manifest
pub fn module_of_file(path: &Path) -> Option<&'static str> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let has = |name: &str| -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            params![name],
            |r| r.get::<_, i32>(0),
        )
        .unwrap_or(0)
            == 1
    };
    if has("phrase_table") {
        Some("phrase")
    } else if has("folder_table") {
        Some("note")
    } else {
        None
    }
}

/// 把设备名清洗成合法、唯一的文件夹名（去掉 Windows/类 Unix 文件系统非法字符）
pub fn sanitize_device(name: &str) -> String {
    let illegal: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
    let mut s: String = name
        .chars()
        .map(|c| if illegal.contains(&c) { '_' } else { c })
        .collect();
    s = s.trim().to_string();
    if s.is_empty() {
        "未知设备".to_string()
    } else {
        s
    }
}

/// 从某个接收文件的 saved_path 反推其所属设备目录
/// （结构固定为 .../_received/{device}/...，取 _received 与 device 这两层）
pub fn device_dir_of(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();
    if let Some(pos) = comps.iter().position(|c| c.as_os_str() == "_received") {
        if pos + 1 < comps.len() {
            return Some(comps[..=pos + 1].iter().collect());
        }
    }
    None
}
