// ============ 导出 ============
// 负责：把 subject 下的 .db 打包为 zip（含 manifest.json），安卓走 SAF。

use serde_json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::path::Path;
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::core::paths::{collect_db_files, get_subject_dir, module_of_file, EXPORTABLE_DIRS};
#[cfg(target_os = "android")]
use crate::core::saf::copy_local_to_content_uri;

#[tauri::command]
pub async fn export_subject_dirs(app: AppHandle, save_path: String) -> Result<usize, String> {
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

    // 1.5 生成 manifest.json：声明每个 db 对应的模块，导入时按 module 字段派发到不同导入函数
    //     note_db.db 与 phrase_db.db 都落在白名单 english 内，二者都会被收集并分别标注模块
    let mut manifest_items: Vec<serde_json::Value> = Vec::new();
    for (rel, path) in &files {
        if let Some(m) = module_of_file(path) {
            manifest_items.push(serde_json::json!({ "module": m, "path": rel }));
        }
    }
    let manifest = serde_json::json!({ "format": 1, "items": manifest_items });
    let manifest_str = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;

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
        // 短语库只导出 source='user' 行（base 种子随 seed 走，不进个人备份/互传）；
        // 其它模块（笔记等）保持整库原样导出。失败则回退整库，不阻断导出。
        let data = if matches!(crate::core::paths::module_of_file(path), Some("phrase")) {
            match crate::english::phrase::export_user_rows_bytes(path) {
                Ok(b) => b,
                Err(e) => {
                    println!("[export] phrase 过滤失败，回退整库导出: {}", e);
                    fs::read(path).map_err(|e| e.to_string())?
                }
            }
        } else {
            fs::read(path).map_err(|e| e.to_string())?
        };
        zip.start_file(rel, options).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }

    // 写入 manifest.json（导入端据此按 module 派发到不同导入函数）
    zip.start_file("manifest.json", options).map_err(|e| e.to_string())?;
    zip.write_all(manifest_str.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;

    // 4. 安卓：把缓存里的 zip 拷到用户选的 content:// 位置
    #[cfg(target_os = "android")]
    if let Some(uri) = content_uri {
        copy_local_to_content_uri(&app, &uri, Path::new(&local_zip_path))?;
        let _ = fs::remove_file(&local_zip_path);
    }

    println!("[export] 已导出 {} 个文件到 {}", files.len(), local_zip_path);
    Ok(files.len())
}
