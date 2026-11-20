# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# ── 冷更新：JNI 反射契约（src-tauri/src/core/update.rs 的 start_install_activity）──
# Rust 侧按「名字」调用下面这些方法，R8 一旦改名就会抛
# NoSuchMethodError: no static method "...getUriForFile(...)"
# （2026-11-20 真机实测：release 包里 getUriForFile 被混淆成 a，安装器拉不起来）。
# 注意：类名因为 AndroidManifest 里声明了 <provider> 会被保留，但成员名不会，
# 所以必须显式 keep 成员。
-keep class androidx.core.content.FileProvider {
    public static android.net.Uri getUriForFile(android.content.Context, java.lang.String, java.io.File);
}
