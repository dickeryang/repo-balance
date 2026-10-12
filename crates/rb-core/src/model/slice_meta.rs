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

/// 工作区文件内容视图：路径 + 字节大小 + 文本内容（可选）。
///
/// 内容读取受编排层预算约束（单文件上限 + 全仓总预算），超限或二进制
/// 文件 `content` 为 `None`（`size` 始终来自元数据，与内容是否读取无关）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileContent {
    /// 相对仓库根的路径（统一正斜杠）。
    pub path: String,
    /// 文件字节大小（元数据，零内容读取也可获得）。
    pub size: u64,
    /// 文件文本内容；超预算 / 二进制 / 读取失败时为 `None`。
    pub content: Option<String>,
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

/// 本地分支拓扑元数据（`git2` 类型不外泄，见 [`crate::git`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchMeta {
    /// 分支名（不含 `refs/heads/` 前缀）。
    pub name: String,
    /// 分支 tip 提交摘要（含 hash / 说明 / 作者 / 时间）。
    pub tip_commit: CommitSummary,
    /// 是否已合并进当前 HEAD（merge_base(tip, HEAD) == tip）。
    pub is_merged: bool,
    /// 是否配置了远端追踪分支（branch.*.remote 存在）。
    pub has_upstream: bool,
    /// 是否为当前检出的 HEAD 分支。
    pub is_current: bool,
}

/// 依赖生态（按清单/锁定文件识别）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ecosystem {
    /// Rust（Cargo.toml / Cargo.lock）。
    Rust,
    /// JavaScript/TypeScript（package.json / package-lock.json / yarn.lock / pnpm-lock.yaml）。
    JavaScript,
    /// Python（pyproject.toml / requirements.txt / poetry.lock / uv.lock / Pipfile）。
    Python,
    /// Go（go.mod / go.sum）。
    Go,
    /// Ruby（Gemfile / Gemfile.lock）。
    Ruby,
    /// PHP（composer.json / composer.lock）。
    Php,
    /// Java/Kotlin（pom.xml / build.gradle，无强制锁定文件）。
    Java,
}

impl Ecosystem {
    /// 全部生态（封闭集）。
    pub const ALL: [Ecosystem; 7] = [
        Ecosystem::Rust,
        Ecosystem::JavaScript,
        Ecosystem::Python,
        Ecosystem::Go,
        Ecosystem::Ruby,
        Ecosystem::Php,
        Ecosystem::Java,
    ];
}

/// 依赖清单文件在依赖体系中的角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepFileKind {
    /// 清单文件（声明依赖）。
    Manifest,
    /// 锁定文件（固定版本）。
    Lock,
}

/// 依赖清单/锁定文件元数据（工作区存在性与 git 跟踪状态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepFileMeta {
    /// 相对仓库根的路径（统一正斜杠）。
    pub path: String,
    /// 所属生态。
    pub ecosystem: Ecosystem,
    /// 清单或锁定文件。
    pub kind: DepFileKind,
    /// 是否被 git 跟踪（存在于 index）。
    pub tracked: bool,
    /// 是否存在于工作区。
    pub in_workdir: bool,
}
