//! 远程克隆：URL → 浅克隆到本地临时目录。
//!
//! 本模块是克隆唯一入口，`git2` 类型不逃逸出 [`git`] 模块。

use std::path::Path;

use crate::error::RbError;
use crate::Result;

/// 浅克隆的默认深度：远程体检只看近期历史，`--depth 1` 足够且传输量最小。
pub const DEFAULT_CLONE_DEPTH: i32 = 1;

/// 把远程仓库浅克隆到 `dest`（须为空目录）。
///
/// `depth` 为浅克隆深度（`1` = 只取最新一次提交）；非正数按默认深度处理。
/// 凭证走当前环境的默认策略（SSH agent / 凭证助手），本 crate 不管理凭证。
///
/// 注意：libgit2 的本地文件传输不支持浅抓取（shallow fetch），因此
/// 本地路径源回退为全量克隆——浅克隆语义只对网络 URL 生效。
pub fn clone_shallow(url: &str, dest: &Path, depth: i32) -> Result<()> {
    let depth = if depth <= 0 { DEFAULT_CLONE_DEPTH } else { depth };

    let mut callbacks = git2::RemoteCallbacks::new();
    // 纯本地语义：克隆过程绝不弹交互式凭证输入，失败直接报错返回。
    callbacks.credentials(|_url, _user, _cred| {
        Err(git2::Error::from_str("远程克隆不支持交互式凭证输入"))
    });
    let mut fetch = git2::FetchOptions::new();
    fetch.remote_callbacks(callbacks);
    if !is_local_path(url) {
        fetch.depth(depth);
    }
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch);
    builder
        .clone(url, dest)
        .map_err(|e| RbError::Git(format!("克隆 {url} 失败: {e}")))?;
    Ok(())
}

/// 是否为本地文件系统路径（非网络协议 URL）。
fn is_local_path(url: &str) -> bool {
    !url.contains("://") && !url.starts_with("git@")
}
