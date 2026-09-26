//! 测试专用：临时 git 仓库夹具构造器（tempfile + git2，固定身份与递增时间戳）。
//!
//! 仅在 `#[cfg(test)]` 下编译；测试代码允许 `unwrap`（纪律只禁非测试代码）。

use git2::{Oid, Repository, Signature, Time};
use tempfile::TempDir;

/// 临时 git 仓库夹具：自动清理，固定身份 `fixture <fixture@test.local>`，
/// 提交时间戳从 1_700_000_000 起每次 +60 秒（保证历史排序稳定）。
pub struct TestRepo {
    pub dir: TempDir,
    pub repo: Repository,
    next_ts: i64,
}

impl TestRepo {
    /// 初始化空仓库。
    pub fn init() -> Self {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        Self {
            dir,
            repo,
            next_ts: 1_700_000_000,
        }
    }

    /// 仓库工作目录路径。
    pub fn path(&self) -> &std::path::Path {
        self.dir.path()
    }

    /// 写入文件并提交（覆盖式），返回提交 oid。
    pub fn commit(&mut self, files: &[(&str, &[u8])], msg: &str) -> Oid {
        let mut index = self.repo.index().unwrap();
        for (path, content) in files {
            let full = self.dir.path().join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&full, content).unwrap();
            index.add_path(std::path::Path::new(path)).unwrap();
        }
        self.commit_tree(&mut index, msg)
    }

    /// 物理删除文件并提交删除（禁用 `--cached`：物理文件必须移除，
    /// 否则工作区扫描会误命中，design 2.1.3.7 关键构造细节）。
    pub fn remove_and_commit(&mut self, path: &str, msg: &str) {
        std::fs::remove_file(self.dir.path().join(path)).unwrap();
        let mut index = self.repo.index().unwrap();
        index.remove_path(std::path::Path::new(path)).unwrap();
        self.commit_tree(&mut index, msg);
    }

    /// 基于当前 HEAD 创建分支（不切换）。
    pub fn branch(&mut self, name: &str) {
        let head = self.repo.head().unwrap().peel_to_commit().unwrap();
        self.repo.branch(name, &head, false).unwrap();
    }

    /// 切换分支并强制检出至工作区。
    pub fn checkout(&mut self, name: &str) {
        let refname = format!("refs/heads/{name}");
        self.repo.set_head(&refname).unwrap();
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.force();
        self.repo.checkout_head(Some(&mut opts)).unwrap();
    }

    fn commit_tree(&mut self, index: &mut git2::Index, msg: &str) -> Oid {
        let tree_id = index.write_tree().unwrap();
        index.write().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let ts = self.next_ts;
        self.next_ts += 60;
        let sig = Signature::new("fixture", "fixture@test.local", &Time::new(ts, 0)).unwrap();
        let parent = match self.repo.head() {
            Ok(head) => Some(self.repo.find_commit(head.target().unwrap()).unwrap()),
            Err(_) => None,
        };
        let parents: Vec<_> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
    }
}