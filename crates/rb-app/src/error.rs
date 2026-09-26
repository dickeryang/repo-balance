//! 错误映射：rb-core RbError → 前端可消费的结构化错误字符串。
//!
//! 映射不得泄露本机绝对路径或 git2 内部细节。

use rb_core::RbError;

/// 将 RbError 映射为前端可消费的中文错误文案。
///
/// 非法 git 仓库 → 「目标不是有效的 git 仓库」；
/// 其余错误返回语义化文案，不泄露路径或 git2 内部细节。
pub fn map_error(e: RbError) -> String {
    match e {
        RbError::InvalidRepo(_) => "目标不是有效的 git 仓库".to_owned(),
        RbError::Git(_) => "Git 仓库读取失败，请检查仓库完整性".to_owned(),
        RbError::Io(_) => "文件系统操作失败，请检查权限与磁盘空间".to_owned(),
        RbError::DuplicateChecker { id } => format!("检查器注册冲突: {id}"),
        RbError::InvalidSlice(_) => "数据切片请求非法".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 非法仓库路径_返回友好文案() {
        let msg = map_error(RbError::InvalidRepo("/secret/path/repo".into()));
        assert!(!msg.contains("/secret/path"));
        assert!(msg.contains("有效的 git 仓库"));
    }

    #[test]
    fn git错误_不泄露内部细节() {
        let msg = map_error(RbError::Git("class=Repository (6); code=NotFound".into()));
        assert!(!msg.contains("class="));
        assert!(!msg.contains("NotFound"));
    }

    #[test]
    fn io错误_不泄露路径() {
        let msg = map_error(RbError::Io(std::io::Error::other("access denied: /secret")));
        assert!(!msg.contains("/secret"));
    }
}