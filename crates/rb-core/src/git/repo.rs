use std::path::Path;

use crate::error::{RbError, Result};
use crate::model::snapshot::{CommitSummary, RepoSnapshot};

/// `git2::Repository` 的只读门面。外界不接触任何 `git2` 类型。
pub struct GitRepo {
    repo: git2::Repository,
}

impl GitRepo {
    /// 打开磁盘上的仓库。
    pub fn open(path: &Path) -> Result<Self> {
        let repo = git2::Repository::discover(path)
            .map_err(|e| RbError::InvalidRepo(format!("{} ({e})", path.display())))?;
        Ok(Self { repo })
    }

    /// 构建只读快照：分支、提交摘要、文件清单、大小与依赖清单文件。
    pub fn snapshot(&self) -> Result<RepoSnapshot> {
        let workdir = self
            .repo
            .workdir()
            .ok_or_else(|| RbError::InvalidRepo("bare 仓库暂不支持".into()))?
            .to_path_buf();

        let branches = self.collect_branches()?;
        let commits = self.collect_commits()?;
        let (files, total_bytes) = collect_files(&workdir);
        let dep_manifests = collect_dep_manifests(&workdir, &files);

        Ok(RepoSnapshot {
            path: workdir.display().to_string(),
            branch_count: branches.len(),
            branches,
            commit_count: commits.len(),
            recent_commits: commits,
            file_count: files.len(),
            total_bytes,
            dep_manifests,
        })
    }

    fn collect_branches(&self) -> Result<Vec<String>> {
        let mut branches = Vec::new();
        let branch_iter = self
            .repo
            .branches(Some(git2::BranchType::Local))
            .map_err(|e| RbError::Git(e.to_string()))?;
        for item in branch_iter {
            let (b, _) = item.map_err(|e| RbError::Git(e.to_string()))?;
            branches.push(
                b.name()
                    .map_err(|e| RbError::Git(e.to_string()))?
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
        branches.sort();
        Ok(branches)
    }

    fn collect_commits(&self) -> Result<Vec<CommitSummary>> {
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|e| RbError::Git(e.to_string()))?;
        revwalk
            .push_head()
            .map_err(|e| RbError::Git(e.to_string()))?;
        revwalk.set_sorting(git2::Sort::TIME).map_err(|e| RbError::Git(e.to_string()))?;

        let mut commits = Vec::new();
        for oid in revwalk.take(20) {
            let oid = oid.map_err(|e| RbError::Git(e.to_string()))?;
            let commit = self
                .repo
                .find_commit(oid)
                .map_err(|e| RbError::Git(e.to_string()))?;
            commits.push(CommitSummary {
                id: commit.id().to_string(),
                summary: commit.summary().unwrap_or_default().to_owned(),
                author: commit
                    .author()
                    .name()
                    .unwrap_or_default()
                    .to_owned(),
                time: commit.time().seconds(),
            });
        }
        Ok(commits)
    }
}

/// 遍历工作目录（跳过 .git），返回文件相对路径清单与总字节数。
fn collect_files(workdir: &Path) -> (Vec<String>, u64) {
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![workdir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
                if let Ok(rel) = path.strip_prefix(workdir) {
                    files.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    files.sort();
    (files, total)
}

/// 常见依赖清单文件名。
const DEP_MANIFEST_NAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "composer.json",
];

fn collect_dep_manifests(workdir: &Path, files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| {
            DEP_MANIFEST_NAMES
                .iter()
                .any(|n| Path::new(f).file_name().is_some_and(|b| b == *n))
        })
        .map(|f| workdir.join(f).display().to_string())
        .collect()
}
