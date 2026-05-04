//! Tauri 命令模块：每个子模块对应一组 IPC 入口。
//!
//! 调用链：前端 `invokeCmd(name, args)` → 本层命令 → `services::*` 业务逻辑
//! → `db::*` 数据访问。命令层只做参数转发 / 错误映射，不写业务。

pub mod dedup;
pub mod empty_dirs;
pub mod mod_tools;
pub mod move_file;
pub mod path;
pub mod pixiv_tag;
pub mod preview;
pub mod records;
pub mod runtime;
pub mod settings;
pub mod suffix;
