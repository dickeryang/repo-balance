//! rb-core — 仓衡检查器内核
//!
//! # 纪律（Hard Rules）
//! 1. 本 crate 不得出现任何 tauri 类型或依赖；`git2` 类型不得逃逸出
//!    [`git`] 模块，对外只暴露自有只读模型（如 [`model::snapshot::RepoSnapshot`]）。
//! 2. 所有公共类型必须 `derive(Debug, Serialize, Deserialize)`，
//!    以便 Tauri IPC 传输与导出 JSON。
//! 3. 错误统一由 [`RbError`]（thiserror）定义；非测试代码禁止
//!    `unwrap()` / `expect()`。
pub mod checker;
pub mod engine;
pub mod error;
pub mod git;
pub mod model;

pub use error::RbError;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, RbError>;
