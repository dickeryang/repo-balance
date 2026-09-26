use thiserror::Error;

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

    /// 检查器 id 重复注册（注册表唯一性校验失败，携带冲突 id）。
    #[error("检查器 id 重复注册: {id}")]
    DuplicateChecker { id: String },

    /// 非法的数据切片请求（如 FileContents 空模式）。
    #[error("非法的数据切片请求: {0}")]
    InvalidSlice(String),
}
