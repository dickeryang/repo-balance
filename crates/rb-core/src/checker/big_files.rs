//! 大文件检查器：工作区大文件（Warning）与历史大文件（Critical，需重写历史）。
//!
//! 纯计算：全部结论由 [`ScanContext`] 中已就绪的切片载荷推导，不触碰文件系统、

//! 不 import git2。`plan` 声明 `FileContents("all")` + `FullHistory` 两切片。

use std::collections::HashMap;

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};
use crate::model::slice_meta::BlobMeta;
use crate::model::snapshot::CommitSummary;

/// 大文件检查器：无状态、纯计算。
pub struct BigFilesChecker;

/// 工作区文件内容切片的固定模式（与 plan 声明一致）。
const ALL_PATTERN: &str = "all";

impl Checker for BigFilesChecker {
    fn id(&self) -> &'static str {
        "big-files"
    }

    fn category(&self) -> Category {
        Category::Structure
    }

    /// 幂等声明两切片：工作区文件元数据 + 全量历史 blob 元数据。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        });
        plan.push(DataSlice::FullHistory);
        plan
    }

    /// 双口径判定：工作区 `size > 阈值` → Warning；历史 `size >= 阈值` 且
    /// 路径不在工作区 → Critical（需重写历史）。切片未就绪时防御性返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let threshold = ctx.config().big_file_threshold;

        let all_slice = DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        };
        let workdir: HashMap<String, u64> = match ctx.slice(&all_slice) {
            Some(SliceData::FileContents(metas)) => {
                metas.iter().map(|m| (m.path.clone(), m.size)).collect()
            }
            _ => return Vec::new(),
        };

        let full_history = DataSlice::FullHistory;
        let history_blobs: Option<&Vec<BlobMeta>> = match ctx.slice(&full_history) {
            Some(SliceData::FullHistory(blobs)) => Some(blobs),
            _ => None,
        };

        // 历史按路径分组：path → (历史最大大小, 最早引入提交)。
        let mut hist_groups: HashMap<String, (u64, CommitSummary)> = HashMap::new();
        if let Some(blobs) = history_blobs {
            for b in blobs {
                hist_groups
                    .entry(b.path.clone())
                    .and_modify(|entry| {
                        if b.size > entry.0 {
                            entry.0 = b.size;
                        }
                        if (b.first_commit.time, b.first_commit.id.as_str())
                            < (entry.1.time, entry.1.id.as_str())
                        {
                            entry.1 = b.first_commit.clone();
                        }
                    })
                    .or_insert((b.size, b.first_commit.clone()));
            }
        }

        let mut findings = Vec::new();
        for (path, &size) in &workdir {
            if size >= threshold {
                findings.push(workdir_finding(path, size, hist_groups.get(path)));
            }
        }
        for (path, (max_size, earliest)) in &hist_groups {
            if *max_size >= threshold && !workdir.contains_key(path) {
                findings.push(history_finding(path, *max_size, earliest));
            }
        }
        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 组装工作区大文件 finding（Warning）。
fn workdir_finding(path: &str, size: u64, hist: Option<&(u64, CommitSummary)>) -> Finding {
    let (hist_max, first_commit) = match hist {
        Some((mx, c)) => (
            human_size(*mx),
            format!("{} {}", short_hash(&c.id), c.summary),
        ),
        None => ("无历史记录".to_owned(), "无历史记录".to_owned()),
    };
    Finding {
        id: format!("big-files.worktree:{path}"),
        severity: Severity::Warning,
        category: Category::Structure,
        title: format!("工作区存在大文件: {path}"),
        evidence: format!(
            "路径: {path}\n当前大小: {}\n历史最大: {hist_max}\n首次引入: {first_commit}",
            human_size(size)
        ),
        suggestion: format!(
            "建议将 {path} 加入 .gitignore，或使用 git lfs migrate import --include=\"{path}\" 迁移到大文件存储"
        ),
    }
}

/// 组装历史大文件 finding（Critical，需重写历史）。
fn history_finding(path: &str, max_size: u64, earliest: &CommitSummary) -> Finding {
    Finding {
        id: format!("big-files.history:{path}"),
        severity: Severity::Critical,
        category: Category::Structure,
        title: format!("历史中存在大文件（删除工作区文件无法解决，需重写历史）: {path}"),
        evidence: format!(
            "路径: {path}\n当前大小: 0 B（已不在工作区）\n历史最大: {}\n首次引入: {} {}",
            human_size(max_size),
            short_hash(&earliest.id),
            earliest.summary
        ),
        suggestion: format!(
            "历史中已无 {path} 但对象仍存在，需重写历史：git filter-repo --path {path} --invert-paths（执行前请备份仓库：git clone --mirror <url> backup.git）"
        ),
    }
}

/// 字节大小的人类可读表达（1024 进制，保留一位小数）。
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// 提交 hash 前 8 个字符（不足则取全部；按字符切片避免 UTF-8 panic）。
fn short_hash(id: &str) -> &str {
    id.get(..id.char_indices().nth(8).map(|(i, _)| i).unwrap_or(id.len())).unwrap_or(id)
}
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::BigFilesChecker;
    use crate::checker::{
        Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData,
    };
    use crate::engine::scan::scan;
    use crate::git::GitRepo;
    use crate::model::finding::Severity;
    use crate::model::slice_meta::{BlobMeta, FileMeta};
    use crate::model::snapshot::{CommitSummary, RepoSnapshot};
    use crate::testutil::TestRepo;

    fn zero_snapshot() -> RepoSnapshot {
        RepoSnapshot {
            path: "/tmp/demo".into(),
            branch_count: 0,
            branches: vec![],
            commit_count: 0,
            recent_commits: vec![],
            file_count: 0,
            total_bytes: 0,
            dep_manifests: vec![],
        }
    }

    fn file_meta(path: &str, size: u64) -> FileMeta {
        FileMeta {
            path: path.into(),
            size,
        }
    }

    fn blob_meta(path: &str, size: u64, cid: &str, summary: &str, time: i64) -> BlobMeta {
        BlobMeta {
            path: path.into(),
            size,
            first_commit: CommitSummary {
                id: cid.into(),
                summary: summary.into(),
                author: "fixture".into(),
                time,
            },
        }
    }

    fn ctx_with<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        workdir: Vec<FileMeta>,
        history: Vec<BlobMeta>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        ctx.insert_slice(
            DataSlice::FileContents {
                pattern: "all".into(),
            },
            SliceData::FileContents(workdir),
        );
        ctx.insert_slice(DataSlice::FullHistory, SliceData::FullHistory(history));
        ctx
    }

    // ===== 7.2 端到端（playbook D1-3 四条场景）=====

    /// ① 工作区大文件 → Warning finding，含路径与当前大小。
    #[test]
    fn 工作区大文件_应产出Warning结论() {
        let mut t = TestRepo::init();
        t.commit(&[("big.bin", &vec![0u8; 2_000_000])], "add big");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(BigFilesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let workdir_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.id.starts_with("big-files.worktree:"))
            .collect();
        assert_eq!(workdir_findings.len(), 1);
        assert_eq!(workdir_findings[0].severity, Severity::Warning);
        assert!(workdir_findings[0].evidence.contains("big.bin"));
    }

    /// ② 仅存在于历史的大文件 → Critical，含「需重写历史」与 git filter-repo。
    #[test]
    fn 仅存在于历史的大文件_应产出Critical结论且含需重写历史文案() {
        let mut t = TestRepo::init();
        t.commit(&[("big.bin", &vec![0u8; 2_000_000])], "add big");
        t.remove_and_commit("big.bin", "remove big");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(BigFilesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let hist_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.id.starts_with("big-files.history:"))
            .collect();
        assert_eq!(hist_findings.len(), 1);
        assert_eq!(hist_findings[0].severity, Severity::Critical);
        assert!(hist_findings[0].title.contains("需重写历史"));
        assert!(hist_findings[0].suggestion.contains("git filter-repo"));
    }

    /// ③ 全部小文件 → 空 findings。
    #[test]
    fn 全部小文件_应返回空结论() {
        let mut t = TestRepo::init();
        t.commit(&[("small.txt", b"hi")], "init");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(BigFilesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }

    /// ④ 阈值注入生效：调小阈值后小文件命中（经采集 + 注入小阈值 config + check）。
    #[test]
    fn 阈值注入生效_调小阈值后小文件命中() {
        let mut t = TestRepo::init();
        t.commit(&[("small.txt", b"hi")], "init");
        let git = GitRepo::open(t.path()).unwrap();
        let snap = git.snapshot().unwrap();
        let workdir = git.workdir_file_metas().unwrap();
        let history = git.history_blob_metas(64, None, None).unwrap();
        let config = ScanConfig {
            big_file_threshold: 1,
        };
        let mut ctx = ScanContext::base(&snap, &config);
        ctx.insert_slice(
            DataSlice::FileContents {
                pattern: "all".into(),
            },
            SliceData::FileContents(workdir),
        );
        ctx.insert_slice(DataSlice::FullHistory, SliceData::FullHistory(history));
        let findings = BigFilesChecker.check(&ctx);
        assert!(findings
            .iter()
            .any(|f| f.id.contains("small.txt") && f.severity == Severity::Warning));
    }

    // ===== 7.3 纯计算边界（口径与契约）=====

    /// ⑤ 工作区文件恰好等于阈值 → 命中（统一 `>=` 口径）。
    #[test]
    fn 阈值边界_工作区文件恰好等于阈值_命中() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 100,
        };
        let ctx = ctx_with(&snap, &config, vec![file_meta("a.txt", 100)], vec![]);
        let findings = BigFilesChecker.check(&ctx);
        assert!(
            findings.iter().any(|f| f.id.contains("a.txt") && f.severity == Severity::Warning),
            "工作区文件恰好等于阈值应命中 Warning"
        );
    }

    /// ⑥ 历史 blob 恰好等于阈值 → 命中（严格 `>=` 口径）。
    #[test]
    fn 阈值边界_历史blob恰好等于阈值_命中() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 100,
        };
        let ctx = ctx_with(
            &snap,
            &config,
            vec![],
            vec![blob_meta("a.txt", 100, "abcdef1234", "init", 1_700_000_000)],
        );
        let findings = BigFilesChecker.check(&ctx);
        assert!(findings
            .iter()
            .any(|f| f.severity == Severity::Critical && f.id.contains("a.txt")));
    }

    /// ⑦ 历史 blob 路径仍在工作区 → 不判 Critical（路径级判断，仅 Warning 一条）。
    #[test]
    fn 历史blob路径仍在工作区_不判Critical() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 100,
        };
        let ctx = ctx_with(
            &snap,
            &config,
            vec![file_meta("a.txt", 200)],
            vec![blob_meta("a.txt", 200, "abcdef1234", "init", 1_700_000_000)],
        );
        let findings = BigFilesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    /// ⑧ finding 字段完整性：evidence 含路径、当前大小、历史最大、首次引入摘要（hash 前 8）。
    #[test]
    fn finding字段完整性_含路径当前大小历史最大与首次引入摘要() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 100,
        };
        let ctx = ctx_with(
            &snap,
            &config,
            vec![file_meta("a.txt", 200)],
            vec![blob_meta("a.txt", 200, "abcdef1234567890", "init commit", 1_700_000_000)],
        );
        let f = &BigFilesChecker.check(&ctx)[0];
        assert!(f.evidence.contains("a.txt"));
        assert!(f.evidence.contains("首次引入"));
        assert!(f.evidence.contains("abcdef12"));
        assert!(f.evidence.contains("init commit"));
    }

    /// ⑨ plan 声明幂等：两次调用等价，切片集合恰为 FileContents("all") + FullHistory。
    #[test]
    fn plan声明幂等_含FileContents与FullHistory两切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = BigFilesChecker.plan(&ctx);
        let p2 = BigFilesChecker.plan(&ctx);
        assert_eq!(p1, p2);
        assert_eq!(p1.slices().len(), 2);
        assert!(p1.slices().contains(&DataSlice::FullHistory));
        assert!(p1
            .slices()
            .iter()
            .any(|s| matches!(s, DataSlice::FileContents { pattern } if pattern == "all")));
    }

    /// ⑩ 同一实例对相同上下文多次 check 返回等价结论列表（纯函数）。
    #[test]
    fn 同一实例重复check结果等价() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 100,
        };
        let ctx = ctx_with(&snap, &config, vec![file_meta("a.txt", 200)], vec![]);
        assert_eq!(BigFilesChecker.check(&ctx), BigFilesChecker.check(&ctx));
    }
}