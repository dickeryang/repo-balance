use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::slice_meta::{
    BlobMeta, BranchMeta, DepFileKind, DepFileMeta, Ecosystem, FileContent, FileMeta,
};
use crate::model::snapshot::{CommitSummary, RepoSnapshot};
use crate::{error::RbError, Result};

/// `git2::Repository` 的只读门面。外界不接触任何 `git2` 类型。
pub struct GitRepo {
    repo: git2::Repository,
}

/// 工作目录遍历结果（内部类型，不外泄）。
struct WorkdirFile {
    /// 相对仓库根的路径（统一正斜杠）。
    rel: String,
    /// 绝对路径。
    abs: std::path::PathBuf,
    /// 字节大小。
    size: u64,
}

/// 二进制判定：前 8 KiB 含 NUL 字节即视为二进制（与 git 的探测口径一致）。
fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|b| *b == 0)
}

/// 按文件名识别依赖清单/锁定文件及其生态；非依赖文件返回 None。
fn classify_dep_file(file_name: &str) -> Option<(Ecosystem, DepFileKind)> {
    Some(match file_name {
        "Cargo.toml" => (Ecosystem::Rust, DepFileKind::Manifest),
        "Cargo.lock" => (Ecosystem::Rust, DepFileKind::Lock),
        "package.json" => (Ecosystem::JavaScript, DepFileKind::Manifest),
        "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => {
            (Ecosystem::JavaScript, DepFileKind::Lock)
        }
        "pyproject.toml" | "requirements.txt" | "Pipfile" => {
            (Ecosystem::Python, DepFileKind::Manifest)
        }
        "poetry.lock" | "uv.lock" | "Pipfile.lock" => (Ecosystem::Python, DepFileKind::Lock),
        "go.mod" => (Ecosystem::Go, DepFileKind::Manifest),
        "go.sum" => (Ecosystem::Go, DepFileKind::Lock),
        "Gemfile" => (Ecosystem::Ruby, DepFileKind::Manifest),
        "Gemfile.lock" => (Ecosystem::Ruby, DepFileKind::Lock),
        "composer.json" => (Ecosystem::Php, DepFileKind::Manifest),
        "composer.lock" => (Ecosystem::Php, DepFileKind::Lock),
        "pom.xml" | "build.gradle" | "build.gradle.kts" => (Ecosystem::Java, DepFileKind::Manifest),
        "gradle.lockfile" => (Ecosystem::Java, DepFileKind::Lock),
        _ => return None,
    })
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
                time_offset: commit.time().offset_minutes(),
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
        let files = self.walk_workdir_files()?;
        Ok(files
            .into_iter()
            .map(|f| FileMeta {
                path: f.rel,
                size: f.size,
            })
            .collect())
    }

    /// 采集工作区文件内容视图：路径 + 大小 + 文本内容（受预算约束）。
    ///
    /// 预算口径（编排层传入，防 pathological 仓库读爆内存/耗时）：
    /// - 单文件超过 `per_file_cap` 字节 → `content = None`（仅元数据）；
    /// - 全仓累计读取达到 `total_budget` 字节 → 后续文件 `content = None`；
    /// - 前 8 KiB 含 NUL 字节判定为二进制 → `content = None`；
    /// - UTF-8 解码失败按 lossy 处理（替换字符），不阻断扫描。
    pub fn workdir_file_contents(
        &self,
        per_file_cap: u64,
        total_budget: u64,
    ) -> Result<Vec<FileContent>> {
        let files = self.walk_workdir_files()?;
        let mut spent = 0u64;
        let mut out = Vec::with_capacity(files.len());
        for f in files {
            let mut content = None;
            if f.size <= per_file_cap && spent < total_budget {
                if let Ok(bytes) = std::fs::read(&f.abs) {
                    if !is_binary(&bytes) {
                        content = Some(String::from_utf8_lossy(&bytes).into_owned());
                        spent += bytes.len() as u64;
                    }
                }
            }
            out.push(FileContent {
                path: f.rel,
                size: f.size,
                content,
            });
        }
        Ok(out)
    }

    /// 遍历工作目录（跳过 .git），返回（相对路径, 绝对路径, 字节数）。
    fn walk_workdir_files(&self) -> Result<Vec<WorkdirFile>> {
        let workdir = self.workdir_path()?;
        let mut files = Vec::new();
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
                        files.push(WorkdirFile {
                            rel: rel.to_string_lossy().replace('\\', "/"),
                            abs: path,
                            size: meta.len(),
                        });
                    }
                }
            }
        }
        files.sort_by(|a, b| a.rel.cmp(&b.rel));
        Ok(files)
    }

    /// 采集本地分支拓扑：tip 提交摘要 + 合并可达性 + 远端追踪 + 是否当前分支。
    ///
    /// HEAD 不存在（空仓库）时 `is_merged` 全部为 false（无合并基准）。
    pub fn branch_graph(&self) -> Result<Vec<BranchMeta>> {
        let head_oid = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok()).map(|c| c.id());
        let branch_iter = self
            .repo
            .branches(Some(git2::BranchType::Local))
            .map_err(|e| RbError::Git(e.to_string()))?;
        let mut out = Vec::new();
        for item in branch_iter {
            let (branch, _) = item.map_err(|e| RbError::Git(e.to_string()))?;
            let Some(name) = branch
                .name()
                .map_err(|e| RbError::Git(e.to_string()))?
                .map(str::to_owned)
            else {
                continue;
            };
            let Some(tip_oid) = branch.get().target() else {
                continue;
            };
            let tip_commit = match self.repo.find_commit(tip_oid) {
                Ok(c) => CommitSummary {
                    id: c.id().to_string(),
                    summary: c.summary().unwrap_or_default().to_owned(),
                    author: c.author().name().unwrap_or_default().to_owned(),
                    time: c.time().seconds(),
                    time_offset: c.time().offset_minutes(),
                },
                Err(_) => continue,
            };
            // 已合并判定：tip 与 HEAD 的 merge_base 即 tip 本身。
            let is_merged = match head_oid {
                Some(head) if head != tip_oid => self
                    .repo
                    .merge_base(tip_oid, head)
                    .map(|base| base == tip_oid)
                    .unwrap_or(false),
                _ => false,
            };
            let has_upstream = branch.upstream().is_ok();
            let is_current = branch.is_head();
            out.push(BranchMeta {
                name,
                tip_commit,
                is_merged,
                has_upstream,
                is_current,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// 采集依赖清单/锁定文件元数据：生态识别 + git 跟踪状态 + 工作区存在性。
    ///
    /// 覆盖工作区内全部已知清单/锁定文件，外加「已跟踪但工作区缺失」的
    /// 锁定文件（如被误删的 Cargo.lock），供依赖健康检查器结构判定。
    pub fn dependency_files(&self) -> Result<Vec<DepFileMeta>> {
        let tracked: std::collections::HashSet<String> = self
            .repo
            .index()
            .map(|idx| {
                idx.iter()
                    .filter_map(|e| std::str::from_utf8(&e.path).ok().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let workdir: std::collections::HashSet<String> = self
            .walk_workdir_files()?
            .into_iter()
            .map(|f| f.rel)
            .collect();

        let mut out: Vec<DepFileMeta> = Vec::new();
        for path in tracked.iter().chain(workdir.iter()) {
            let Some(file_name) = std::path::Path::new(path).file_name().and_then(|n| n.to_str())
            else {
                continue;
            };
            let Some((ecosystem, kind)) = classify_dep_file(file_name) else {
                continue;
            };
            if out.iter().any(|d| d.path == *path) {
                continue;
            }
            out.push(DepFileMeta {
                path: path.clone(),
                ecosystem,
                kind,
                tracked: tracked.contains(path),
                in_workdir: workdir.contains(path),
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
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
            time_offset: commit.time().offset_minutes(),
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
#[allow(non_snake_case)]
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

    /// 分支拓扑：当前分支标记 is_current；祖先分支已合并；有新提交的分支未合并；
    /// 无远端配置时 has_upstream 均为 false。
    #[test]
    fn 分支拓扑_合并状态与当前分支标记() {
        use crate::model::slice_meta::BranchMeta;
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "c1");
        t.branch("feature"); // tip = c1，之后 main 前进 → 已合并
        t.commit(&[("b.txt", b"hi")], "c2 on main");
        t.branch("old");
        t.checkout("old");
        t.commit(&[("c.txt", b"hi")], "c3 on old"); // 未合并
        t.checkout("main");
        let git = GitRepo::open(t.path()).unwrap();
        let branches = git.branch_graph().unwrap();
        let by_name = |name: &str| -> BranchMeta {
            branches.iter().find(|b| b.name == name).cloned().unwrap()
        };
        assert_eq!(branches.len(), 3);
        assert!(by_name("main").is_current);
        assert!(!by_name("feature").is_current);
        assert!(by_name("feature").is_merged, "祖先分支应判定为已合并");
        assert!(!by_name("old").is_merged, "含未合并提交的分支不应判定已合并");
        for b in &branches {
            assert!(!b.has_upstream, "夹具无远端配置，均无远端追踪");
            assert_eq!(b.tip_commit.id.len(), 40);
        }
    }

    /// 工作区内容采集：文本文件有内容；二进制（含 NUL）与超单文件上限为 None，
    /// 但 size 元数据始终齐全。
    #[test]
    fn 工作区内容采集_文本有内容_二进制与超限为None() {
        use crate::model::slice_meta::FileContent;
        let mut t = TestRepo::init();
        t.commit(&[("text.txt", b"hello")], "init");
        let mut bin = vec![0u8; 32];
        bin[0] = b'O';
        bin[1] = 0;
        t.commit(&[("blob.bin", &bin)], "add binary");
        t.commit(&[("big.txt", &[b'x'; 64])], "add big");
        let git = GitRepo::open(t.path()).unwrap();
        let files = git.workdir_file_contents(32, 1_048_576).unwrap();
        let by_path = |name: &str| -> FileContent {
            files.iter().find(|f| f.path == name).cloned().unwrap()
        };
        assert_eq!(by_path("text.txt").content.as_deref(), Some("hello"));
        assert_eq!(by_path("blob.bin").content, None, "二进制文件不读取内容");
        assert_eq!(by_path("blob.bin").size, 32, "二进制文件 size 仍记录");
        assert_eq!(by_path("big.txt").content, None, "超单文件上限不读取内容");
        assert_eq!(by_path("big.txt").size, 64, "超限文件 size 仍记录");
    }

    /// 工作区内容采集：全仓总预算耗尽后后续文件 content 为 None（预算保护）。
    #[test]
    fn 工作区内容采集_总预算耗尽后不再读取() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", &[b'a'; 16])], "c1");
        t.commit(&[("b.txt", &[b'b'; 16])], "c2");
        let git = GitRepo::open(t.path()).unwrap();
        let files = git.workdir_file_contents(1024, 16).unwrap();
        let read = files.iter().filter(|f| f.content.is_some()).count();
        assert!(read <= 2, "预算内最多读满即止");
        let total_read: usize = files
            .iter()
            .filter_map(|f| f.content.as_ref().map(|c| c.len()))
            .sum();
        assert!(total_read <= 16, "累计读取不得超过总预算");
    }

    /// 依赖文件采集：识别生态与角色；git 跟踪状态与工作区存在性分开标记；
    /// 覆盖「已跟踪但工作区缺失」的锁定文件。
    #[test]
    fn 依赖文件采集_生态角色与跟踪状态() {
        use crate::model::slice_meta::{DepFileKind, Ecosystem};
        let mut t = TestRepo::init();
        t.commit(
            &[("Cargo.toml", b"[package]\nname=\"x\"\n"), ("Cargo.lock", b"# lock\n")],
            "init",
        );
        std::fs::write(t.path().join("package.json"), b"{}").unwrap(); // 未跟踪
        let git = GitRepo::open(t.path()).unwrap();
        let deps = git.dependency_files().unwrap();
        let by_path = |name: &str| deps.iter().find(|d| d.path == name).cloned().unwrap();
        let lock = by_path("Cargo.lock");
        assert_eq!(lock.ecosystem, Ecosystem::Rust);
        assert_eq!(lock.kind, DepFileKind::Lock);
        assert!(lock.tracked && lock.in_workdir);
        let pkg = by_path("package.json");
        assert_eq!(pkg.ecosystem, Ecosystem::JavaScript);
        assert!(!pkg.tracked, "未加入 index 的文件 tracked 为 false");
        assert!(pkg.in_workdir);

        // 已跟踪但工作区缺失的锁定文件（未 staged 的物理删除）仍被采集。
        std::fs::remove_file(t.path().join("Cargo.lock")).unwrap();
        let git = GitRepo::open(t.path()).unwrap();
        let deps = git.dependency_files().unwrap();
        let lock = deps.iter().find(|d| d.path == "Cargo.lock").unwrap();
        assert!(lock.tracked, "仍在 index 中，tracked 应为 true");
        assert!(!lock.in_workdir, "工作区已删除，in_workdir 应为 false");
    }
}
