// ========== Android: content:// URI 与本地文件互转 ==========
// 负责：安卓 SAF 场景下，把 content:// URI 拷到应用缓存（导入用），
// 以及把本地文件拷到 content:// URI 指向的位置（导出用）。

#[cfg(target_os = "android")]
use std::fs;
#[cfg(target_os = "android")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "android")]
use tauri::AppHandle;

/// 把 content:// URI 指向的文件拷到应用缓存目录，返回本地真实路径（导入用）
#[cfg(target_os = "android")]
pub fn content_uri_to_cache(
    _app: &AppHandle,
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
        use std::io::Write as _;
        out.write_all(&bytes[..n as usize])
            .map_err(|e| e.to_string())?;
    }
    env.call_method(&stream, "close", "()V", &[])
        .map_err(|e| e.to_string())?;

    Ok(target)
}

/// 把本地文件拷贝到 content:// URI 指向的位置（导出用）
#[cfg(target_os = "android")]
pub fn copy_local_to_content_uri(
    _app: &AppHandle,
    uri: &str,
    local_path: &Path,
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
