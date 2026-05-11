//! 领域数据模型。
//!
//! 所有结构体统一使用 `#[serde(rename_all = "camelCase")]`，
//! 以匹配前端 TypeScript 侧的字段命名约定。子模块按领域划分；
//! 顶层重导出以保持 `use crate::models::X` 兼容既有代码。

pub mod empty_dirs;
pub mod hash_record;
pub mod image_dedup;
pub mod mod_tools;
pub mod move_file;
pub mod path_norm;
pub mod pixiv_tag;
pub mod settings;
pub mod suffix;
pub mod task;

pub use empty_dirs::*;
pub use hash_record::*;
pub use image_dedup::*;
pub use mod_tools::*;
pub use move_file::*;
pub use path_norm::*;
pub use pixiv_tag::*;
pub use settings::*;
pub use suffix::*;
pub use task::*;
