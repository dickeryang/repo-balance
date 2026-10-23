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
///
/// 传输支持取决于编译期 feature：本 crate 开启 git2 的 `https`
/// （macOS = SecureTransport / Windows = WinHTTP / Linux = 系统 OpenSSL），
/// 未开启 `ssh`，故 `ssh://` 与未配置 `insteadOf` 的 `git@host:` 地址会失败；
/// 这类失败会给出「改用 HTTPS」的可执行提示，而不是原样抛出 libgit2 的
/// `unsupported URL protocol`。
pub fn clone_shallow(url: &str, dest: &Path, depth: i32) -> Result<()> {
    let depth = if depth <= 0 {
        DEFAULT_CLONE_DEPTH
    } else {
        depth
    };

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
    if let Err(e) = builder.clone(url, dest) {
        // SSH 传输未编译进来：libgit2 只给一句 `unsupported URL protocol`，
        // 用户据此无从下手，这里替换成「该怎么做」的提示。
        if is_ssh_url(url) {
            return Err(RbError::Git(format!(
                "克隆 {url} 失败: 当前构建不支持 SSH 传输，请改用 HTTPS 地址；\
                 或在 git 配置里用 url.<HTTPS 前缀>.insteadOf 把 SSH 地址改写成 HTTPS"
            )));
        }
        return Err(RbError::Git(format!("克隆 {url} 失败: {e}")));
    }
    Ok(())
}

/// 是否为本地文件系统路径（非网络协议 URL）。
fn is_local_path(url: &str) -> bool {
    !url.contains("://") && !url.starts_with("git@")
}

/// 是否为 SSH 地址（`ssh://` / `git+ssh://`，或 `git@host:path` 这种 scp 写法）。
///
/// 注：`git@host:` 若被 git 配置的 `url.*.insteadOf` 改写成 HTTPS 会正常克隆，
/// 此时不会走到这里的失败分支。
fn is_ssh_url(url: &str) -> bool {
    url.starts_with("ssh://")
        || url.starts_with("git+ssh://")
        || (!url.contains("://") && url.contains('@'))
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{is_local_path, is_ssh_url};

    /// URL 形态判定：决定浅抓取是否生效，以及失败时给哪一类提示。
    #[test]
    fn url_形态判定_区分本地路径与网络地址() {
        assert!(is_local_path("/tmp/repo"));
        assert!(is_local_path("./repo"));
        assert!(!is_local_path("https://example.com/a.git"));
        assert!(!is_local_path("git@github.com:u/r.git"));
    }

    /// SSH 判定覆盖两种写法；HTTPS 与本地路径不能被误判为 SSH。
    #[test]
    fn url_形态判定_识别SSH地址() {
        assert!(is_ssh_url("ssh://git@github.com/u/r.git"));
        assert!(is_ssh_url("git+ssh://git@github.com/u/r.git"));
        assert!(is_ssh_url("git@github.com:u/r.git"));
        assert!(!is_ssh_url("https://github.com/u/r.git"));
        assert!(!is_ssh_url("/tmp/repo"));
    }
}
