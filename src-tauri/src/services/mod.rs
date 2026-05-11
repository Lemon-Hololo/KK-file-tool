//! 业务逻辑层：commands 与 db 之间的桥梁。
//!
//! - [`events`] 发事件
//! - [`logging`] 封装 `task_log` 上下文
//! - [`op_pipeline`] 通用 preview → apply → rollback 流水线
//! - [`suffix`] / [`empty_dirs`] / [`mod_tools`] 记录型操作
//! - [`dedup`] / [`move_file`] / [`preview`] 一次性业务

pub mod dedup;
pub mod empty_dirs;
pub mod events;
pub mod image_dedup;
pub mod logging;
pub mod mod_tools;
pub mod move_file;
pub mod op_pipeline;
pub mod pixiv_tag;
pub mod preview;
pub mod suffix;
