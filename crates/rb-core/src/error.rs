use thiserror::Error;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, RbError>;

/// 统一错误类型。
#[derive(Debug, Error)]
pub enum RbError {
    /// git 仓库打开或读取失败（git2 细节不逃逸出 git 模块，仅保留消息）。
    #[error("git 错误: {0}")]
    Git(String),

    /// 目标路径不存在或不是有效仓库。
    #[error("无效的仓库路径: {0}")]
    InvalidRepo(String),

    /// IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}
