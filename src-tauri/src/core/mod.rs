// core 模块：把原 lib.rs 中按职责拆分出的子模块统一归集。
//
// 子模块划分：
//   paths   —— subject 目录、db 扫描/收集、模块嗅探、设备名清洗（共享基础设施）
//   device  —— 设备名、mDNS 发现/广播、相关 State
//   receive —— 接收 HTTP 服务、上传落盘、接收开关
//   send    —— 局域网发送
//   import  —— zip 导入 + 局域网接收导入 + 接收缓存清理
//   export  —— 导出为 zip
//   saf     —— 安卓 content:// URI 互转
//   init    —— 首次启动环境初始化 + 基础词 db 迁移（种子 resource → 工作库）
//
// 各子模块的导出项由 lib.rs 通过完整路径（crate::core::*::*）引用与注册。

pub mod paths;
pub mod device;
pub mod receive;
pub mod send;
pub mod import;
pub mod export;
pub mod saf;
pub mod config;
pub mod init;

