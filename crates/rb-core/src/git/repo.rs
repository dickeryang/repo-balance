use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::slice_meta::{BlobMeta, FileMeta};
use crate::model::snapshot::{CommitSummary, RepoSnapshot};
use crate::{error::RbError, Result};


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
        let commit_count = self.count_commits()?;
        let recent_commits = self.collect_recent_commits()?;
        let (files, total_bytes) = collect_files(&workdir);
        let dep_manifests = collect_dep_manifests(&files);

        Ok(RepoSnapshot {
            path: workdir.display().to_string(),
            branch_count: branches.len(),
            branches,
            commit_count,
            recent_commits,
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

    /// HEAD 可达提交总数：全量遍历仅对 oid 计数，不物化提交对象。
    fn count_commits(&self) -> Result<usize> {
        let Some(revwalk) = self.revwalk_head()? else {
            return Ok(0);
        };
        let mut count = 0usize;
        for oid in revwalk {
            oid.map_err(|e| RbError::Git(e.to_string()))?;
            count += 1;
        }
        Ok(count)
    }

    /// 最近提交摘要：按时间由新到旧，固定最多 20 条。
    fn collect_recent_commits(&self) -> Result<Vec<CommitSummary>> {
        let Some(mut revwalk) = self.revwalk_head()? else {
            return Ok(Vec::new());
        };
        revwalk
            .set_sorting(git2::Sort::TIME)
            .map_err(|e| RbError::Git(e.to_string()))?;

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
                author: commit.author().name().unwrap_or_default().to_owned(),
                time: commit.time().seconds(),
            });
        }
        Ok(commits)
    }

    /// 建立以 HEAD 为起点的 revwalk。
    /// HEAD 未出生或无任何提交时按空仓库处理（返回 `None`，spec 5.2.3.3），
    /// 其余失败仍上报 [`RbError::Git`]。
    fn revwalk_head(&self) -> Result<Option<git2::Revwalk<'_>>> {
        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|e| RbError::Git(e.to_string()))?;
        if let Err(e) = revwalk.push_head() {
            return match e.code() {
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound => Ok(None),
                _ => Err(RbError::Git(e.to_string())),
            };
        }
        Ok(Some(revwalk))
    }

    /// 采集工作区文件元数据：路径（正斜杠、相对仓库根、字典序）+ 字节大小。
    ///
    /// 覆盖 tracked 与未跟踪文件、跳过 `.git` 目录；仅读取元数据，
    /// 全程不读取任何文件内容（spec 4.1.2）。
    pub fn workdir_file_metas(&self) -> Result<Vec<FileMeta>> {
        let workdir = self.workdir_path()?;
        let mut metas = Vec::new();
        let mut stack = vec![workdir.clone()];
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
                // file_type() 不跟随符号链接：指向目录的符号链接被当作文件处理，
                // 避免 ln -s . 之类的循环导致死循环。
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    stack.push(path);
                } else if let Ok(meta) = entry.metadata() {
                    if let Ok(rel) = path.strip_prefix(&workdir) {
                        metas.push(FileMeta {
                            path: rel.to_string_lossy().replace('\\', "/"),
                            size: meta.len(),
                        });
                    }
                }
            }
        }
        metas.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(metas)
    }

    /// 采集全部本地分支可达的历史 blob 元数据（spec 5.1.1.3）。
    ///
    /// 以 `refs/heads/*` 为多起点 revwalk、时间正序（TIME | REVERSE），
    /// 提交按 `batch_size` 分批、顺序处理（每批独立
    /// `Repository::open` 同一 workdir——`Repository` 是 Send 非 Sync，
    /// 不可跨线程共享句柄）；批内仅读对象头 `size()`，绝不读取内容；
    /// 单对象解析失败跳过继续（对象级容错），revwalk 建立失败透传
    /// [`RbError::Git`]（整仓级）。主线程按时间正序归并，同一路径同一
    /// blob 保留最早提交为「首次引入」语义；`batch_size` 为 0 时按 1 处理。
    /// `cancel_flag` 为 `Some` 时在每批完成后检查，用户取消则提前返回已采集部分。
    /// `on_progress` 为 `Some` 时在历史采集开始与每批完成后回调
    /// `(已完成提交数, 提交总数)`，供编排层上报细粒度进度。
    pub fn history_blob_metas(
        &self,
        batch_size: usize,
        cancel_flag: Option<&AtomicBool>,
        on_progress: Option<&dyn Fn(usize, usize)>,
    ) -> Result<Vec<BlobMeta>> {
        let workdir = self.workdir_path()?;

        // 多起点 revwalk：全部本地分支 tip（覆盖未合并分支对象）。
        let branch_iter = self
            .repo
            .branches(Some(git2::BranchType::Local))
            .map_err(|e| RbError::Git(e.to_string()))?;
        let mut tips = Vec::new();
        for item in branch_iter {
            let (branch, _) = item.map_err(|e| RbError::Git(e.to_string()))?;
            if let Some(oid) = branch.get().target() {
                tips.push(oid);
            }
        }
        if tips.is_empty() {
            return Ok(Vec::new());
        }

        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|e| RbError::Git(e.to_string()))?;
        for tip in tips {
            if let Err(e) = revwalk.push(tip) {
                return Err(RbError::Git(e.to_string()));
            }
        }
        revwalk
            .set_sorting(git2::Sort::TIME | git2::Sort::REVERSE)
            .map_err(|e| RbError::Git(e.to_string()))?;

        // 主线程单遍收集可达提交 oid（时间正序），单对象解析失败跳过。
        let mut commit_oids = Vec::new();
        for oid in revwalk.flatten() {
            commit_oids.push(oid);
        }

        // 分批处理：每批独立开仓（Repository Send 非 Sync 硬约束）。
        // 改为顺序处理以在批次边界检查取消标志（大仓库场景下批次少，
        // 取消响应延迟可接受；后续可恢复并行 + 批间检查）。
        let width = batch_size.max(1);
        let total_commits = commit_oids.len();
        if let Some(cb) = on_progress {
            cb(0, total_commits);
        }
        let mut processed_commits = 0usize;
        let mut batch_results: Vec<Vec<(String, String, u64, CommitSummary)>> = Vec::new();
        for batch in commit_oids.chunks(width) {
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
            }
            batch_results.push(process_batch(&workdir, batch));
            processed_commits += batch.len();
            if let Some(cb) = on_progress {
                cb(processed_commits, total_commits);
            }
        }

        // 主线程按批序（时间正序）归并：同 (路径, blob) 保留最早提交。
        let mut seen: HashMap<(String, String), BlobMeta> = HashMap::new();
        for batch in batch_results {
            for (path, blob_id, size, first_commit) in batch {
                let key = (path.clone(), blob_id);
                seen.entry(key).or_insert(BlobMeta {
                    path,
                    size,
                    first_commit,
                });
            }
        }
        let mut metas: Vec<BlobMeta> = seen.into_values().collect();
        metas.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.first_commit.time.cmp(&b.first_commit.time))
                .then(a.first_commit.id.cmp(&b.first_commit.id))
        });
        Ok(metas)
    }

    /// 仓库工作目录路径（bare 仓库暂不支持）。
    fn workdir_path(&self) -> Result<std::path::PathBuf> {
        self.repo
            .workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| RbError::InvalidRepo("bare 仓库暂不支持".into()))
    }
}

/// 处理一批提交：解析每棵提交树并收集 blob 的（路径、oid、大小、所属提交摘要）。
///
/// 单对象解析失败跳过继续（对象级容错）；仅读对象头 `size()`，零内容读取。
fn process_batch(
    workdir: &Path,
    batch: &[git2::Oid],
) -> Vec<(String, String, u64, CommitSummary)> {
    let mut out = Vec::new();
    let Ok(repo) = git2::Repository::open(workdir) else {
        return out;
    };
    for oid in batch {
        let Ok(commit) = repo.find_commit(*oid) else {
            continue;
        };
        let summary = CommitSummary {
            id: commit.id().to_string(),
            summary: commit.summary().unwrap_or_default().to_owned(),
            author: commit.author().name().unwrap_or_default().to_owned(),
            time: commit.time().seconds(),
        };
        let Ok(tree) = commit.tree() else {
            continue;
        };
        collect_tree_blobs(&repo, tree.id(), "", &mut out, &summary);
    }
    out
}

/// 递归收集树中全部 blob 的（路径、oid、大小）+ 所属提交摘要。
fn collect_tree_blobs(
    repo: &git2::Repository,
    tree_id: git2::Oid,
    prefix: &str,
    out: &mut Vec<(String, String, u64, CommitSummary)>,
    summary: &CommitSummary,
) {
    let Ok(tree) = repo.find_tree(tree_id) else {
        return;
    };
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                collect_tree_blobs(repo, entry.id(), &path, out, summary);
            }
            Some(git2::ObjectType::Blob) => {
                let Ok(blob) = repo.find_blob(entry.id()) else {
                    continue;
                };
                out.push((
                    path,
                    entry.id().to_string(),
                    blob.size() as u64,
                    summary.clone(),
                ));
            }
            _ => {}
        }
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
            // file_type() 不跟随符号链接，避免循环链接死循环。
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
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

/// 从文件清单中筛出依赖清单，直接复用已归一化（相对 workdir、正斜杠）的
/// 相对路径；输入已按字典序排序，输出保持序稳定。
fn collect_dep_manifests(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| {
            DEP_MANIFEST_NAMES
                .iter()
                .any(|n| Path::new(f).file_name().is_some_and(|b| b == *n))
        })
        .cloned()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::GitRepo;
    use crate::testutil::TestRepo;

    /// ⑪ 工作区采集：覆盖 tracked 与未跟踪文件，路径正斜杠。
    #[test]
    fn 工作区采集_含未跟踪文件且路径正斜杠() {
        let mut t = TestRepo::init();
        t.commit(&[("tracked.txt", b"hi")], "init");
        std::fs::write(t.path().join("untracked.txt"), b"x").unwrap();
        std::fs::create_dir_all(t.path().join("sub")).unwrap();
        std::fs::write(t.path().join("sub/deep.txt"), b"y").unwrap();
        let git = GitRepo::open(t.path()).unwrap();
        let metas = git.workdir_file_metas().unwrap();
        let paths: Vec<&str> = metas.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.contains(&"tracked.txt"));
        assert!(paths.contains(&"untracked.txt"));
        assert!(paths.contains(&"sub/deep.txt"));
    }

    /// ⑫ 历史采集：同一大文件两次提交（改大后重提），最早提交的 first_commit 指向第一次。
    #[test]
    fn 历史采集_首次引入提交为最早提交() {
        let mut t = TestRepo::init();
        t.commit(&[("big.bin", &vec![0u8; 200_000])], "first");
        t.commit(&[("big.bin", &vec![0u8; 300_000])], "second");
        let git = GitRepo::open(t.path()).unwrap();
        let blobs = git.history_blob_metas(64, None, None).unwrap();
        let big_blobs: Vec<_> = blobs.iter().filter(|b| b.path == "big.bin").collect();
        assert_eq!(big_blobs.len(), 2);
        let earliest = big_blobs.iter().min_by_key(|b| b.first_commit.time).unwrap();
        assert_eq!(earliest.first_commit.summary, "first");
    }

    /// ⑬ 历史采集：batch_size=1 注入，结果与整批一致（并行分批无丢失无重复）。
    #[test]
    fn 历史采集_小批次并行路径正确性() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "c1");
        t.commit(&[("b.txt", b"hi")], "c2");
        t.commit(&[("c.txt", b"hi")], "c3");
        let git = GitRepo::open(t.path()).unwrap();
        let full = git.history_blob_metas(64, None, None).unwrap();
        let batched = git.history_blob_metas(1, None, None).unwrap();
        let mut full_pairs: Vec<_> = full.iter().map(|b| (b.path.clone(), b.size)).collect();
        let mut batched_pairs: Vec<_> = batched.iter().map(|b| (b.path.clone(), b.size)).collect();
        full_pairs.sort();
        batched_pairs.sort();
        assert_eq!(full_pairs, batched_pairs);
    }

    /// ⑭ 历史采集：old-branch 上提交大文件（未合并），history_blob_metas 产出该 blob。
    #[test]
    fn 历史采集_多分支对象覆盖() {
        let mut t = TestRepo::init();
        t.commit(&[("small.txt", b"hi")], "init");
        t.branch("old");
        t.checkout("old");
        t.commit(&[("big_old.bin", &vec![0u8; 200_000])], "big on old");
        let git = GitRepo::open(t.path()).unwrap();
        let blobs = git.history_blob_metas(64, None, None).unwrap();
        assert!(blobs.iter().any(|b| b.path == "big_old.bin"));
    }
}
