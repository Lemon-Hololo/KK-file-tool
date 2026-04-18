//! 业务逻辑层：commands 与 db 之间的桥梁。
//!
//! - [`events`] 发事件
//! - [`op_pipeline`] 通用 preview → apply → rollback 流水线
//! - [`suffix`] / [`mod_tools`] 两种记录型操作
//! - [`dedup`] / [`move_file`] / [`preview`] 一次性业务

pub mod dedup;
pub mod events;
pub mod mod_tools;
pub mod move_file;
pub mod op_pipeline;
pub mod preview;
pub mod suffix;
