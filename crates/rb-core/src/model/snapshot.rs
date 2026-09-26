use serde::{Deserialize, Serialize};

/// 单条提交的摘要信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    /// 提交 SHA（完整 40 位）。
    pub id: String,
    /// 首行提交说明。
    pub summary: String,
    /// 作者名。
    pub author: String,
    /// Unix 时间戳（秒）。
    pub time: i64,
}

/// 仓库只读快照：对外暴露的唯一 git 状态视图。
///
/// 构建于 [`crate::git::GitRepo`]，`git2` 类型不外泄。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    /// 仓库工作目录绝对路径。
    pub path: String,
    /// 本地分支数。
    pub branch_count: usize,
    /// 本地分支名（排序后）。
    pub branches: Vec<String>,
    /// 提交总数（HEAD 可达）。
    pub commit_count: usize,
    /// 最近提交摘要（按时间倒序，最多 20 条）。
    pub recent_commits: Vec<CommitSummary>,
    /// 工作区文件数量（含未跟踪文件，跳过 .git 目录）。
    pub file_count: usize,
    /// 工作目录文件总字节数（不含 .git）。
    pub total_bytes: u64,
    /// 依赖清单文件相对路径（相对仓库根，统一正斜杠，字典序稳定）。
    pub dep_manifests: Vec<String>,
}
