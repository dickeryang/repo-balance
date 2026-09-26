use serde::{Deserialize, Serialize};

use crate::model::snapshot::CommitSummary;

/// 工作区文件元数据：路径 + 字节大小（不含内容字节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMeta {
    /// 相对仓库根的路径（统一正斜杠）。
    pub path: String,
    /// 文件字节大小。
    pub size: u64,
}

/// 历史 blob 元数据：路径 + 字节大小 + 首次引入提交摘要（不含内容字节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobMeta {
    /// 相对仓库根的路径（统一正斜杠）。
    pub path: String,
    /// blob 字节大小（取自对象头，零内容读取）。
    pub size: u64,
    /// 该 blob 首次引入时的提交摘要。
    pub first_commit: CommitSummary,
}