//! git 模块：`git2` 的唯一使用点。`git2` 类型不得逃逸出本模块。

mod repo;

pub use repo::GitRepo;
