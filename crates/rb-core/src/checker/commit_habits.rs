//! 提交习惯检查器（History 维度）：超大提交、低质提交信息占比、深夜提交比例。
//!
//! 纯计算：消费 `FullHistory` 切片（按首次引入提交聚合文件数）+ 快照的
//! `recent_commits`（最近 20 条提交摘要），不触碰 git。

use std::collections::HashMap;

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};
use crate::model::snapshot::CommitSummary;
use crate::timefmt::{format_utc, local_hour};

/// 提交习惯检查器：无状态、纯计算。
pub struct CommitHabitsChecker;

/// 超大提交 Warning 阈值（引入文件数，严格 >）。
const OVERSIZED_WARNING: usize = 50;

/// 超大提交 Critical 阈值（引入文件数，严格 >）。
const OVERSIZED_CRITICAL: usize = 200;

/// 比例类规则的最小样本量（最近提交数不足时不判定，避免小样本误导）。
const RATIO_MIN_SAMPLE: usize = 5;

/// 比例类规则触发阈值（严格 >）。
const RATIO_THRESHOLD: f64 = 0.3;

/// 深夜时段定义：当地 23:00-06:00。
const NIGHT_HOURS: [u32; 2] = [23, 6];

impl Checker for CommitHabitsChecker {
    fn id(&self) -> &'static str {
        "commit-habits"
    }

    fn category(&self) -> Category {
        Category::History
    }

    /// 幂等声明全量历史切片（用于按提交聚合引入文件数）。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::FullHistory);
        plan
    }

    /// 三类规则；快照无最近提交（空仓库）时返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let recent = &ctx.snapshot().recent_commits;
        if recent.is_empty() {
            return Vec::new();
        }

        // 文件数聚合：仅统计最近提交集合内首次引入的 (路径, blob)。
        let mut files_per_commit: HashMap<&str, usize> = HashMap::new();
        if let Some(SliceData::FullHistory(blobs)) = ctx.slice(&DataSlice::FullHistory) {
            let recent_ids: std::collections::HashSet<&str> =
                recent.iter().map(|c| c.id.as_str()).collect();
            for b in blobs {
                if recent_ids.contains(b.first_commit.id.as_str()) {
                    *files_per_commit
                        .entry(b.first_commit.id.as_str())
                        .or_insert(0) += 1;
                }
            }
        }

        let mut findings = Vec::new();
        findings.extend(oversized_findings(recent, &files_per_commit));
        findings.extend(ratio_findings(recent));
        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 规则一：超大提交（引入文件数超阈值）。
fn oversized_findings(
    recent: &[CommitSummary],
    files_per_commit: &HashMap<&str, usize>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for c in recent {
        let Some(&files) = files_per_commit.get(c.id.as_str()) else {
            continue;
        };
        let (severity, threshold) = if files > OVERSIZED_CRITICAL {
            (Severity::Critical, OVERSIZED_CRITICAL)
        } else if files > OVERSIZED_WARNING {
            (Severity::Warning, OVERSIZED_WARNING)
        } else {
            continue;
        };
        out.push(Finding {
            id: format!("commit-habits.oversized:{}", short_hash(&c.id)),
            severity,
            category: Category::History,
            title: format!("超大提交（引入 {files} 个文件）: {}", c.summary),
            evidence: format!(
                "提交: {}\n说明: {}\n时间: {}\n引入文件数: {files}（阈值 {threshold}）",
                short_hash(&c.id),
                c.summary,
                format_utc(c.time)
            ),
            suggestion: "拆分该提交：用 git add -p 交互式暂存把无关改动分到多个提交；确需整体提交的（如代码生成）可在 .repobalance-ignore 忽略对应路径".to_owned(),
        });
    }
    out
}

/// 规则二/三：低质信息占比、深夜提交比例（聚合 finding）。
fn ratio_findings(recent: &[CommitSummary]) -> Vec<Finding> {
    let total = recent.len();
    if total < RATIO_MIN_SAMPLE {
        return Vec::new();
    }

    let mut out = Vec::new();
    let low: Vec<&CommitSummary> = recent
        .iter()
        .filter(|c| is_low_quality(&c.summary))
        .collect();
    let low_ratio = low.len() as f64 / total as f64;
    if low_ratio > RATIO_THRESHOLD {
        let pct = (low_ratio * 100.0).round() as u32;
        let examples: Vec<String> = low
            .iter()
            .take(3)
            .map(|c| format!("「{}」", c.summary))
            .collect();
        out.push(Finding {
            id: "commit-habits.low-quality-ratio".to_owned(),
            severity: Severity::Warning,
            category: Category::History,
            title: format!("低质提交信息占比过高: {pct}%"),
            evidence: format!(
                "最近 {total} 条提交中 {low_n} 条为低质信息（wip / fix typo / 过短等，占 {pct}%）\n示例: {examples}",
                low_n = low.len(),
                examples = examples.join("、")
            ),
            suggestion: "采用约定式提交（如 feat: / fix: / refactor: 前缀 + 简明描述），合并前的 wip 提交用 git rebase -i 清理为单一说明".to_owned(),
        });
    }

    let night: Vec<&CommitSummary> = recent
        .iter()
        .filter(|c| is_night(c.time, c.time_offset))
        .collect();
    let night_ratio = night.len() as f64 / total as f64;
    if night_ratio > RATIO_THRESHOLD {
        let pct = (night_ratio * 100.0).round() as u32;
        out.push(Finding {
            id: "commit-habits.night-ratio".to_owned(),
            severity: Severity::Info,
            category: Category::History,
            title: format!("深夜提交占比过高: {pct}%"),
            evidence: format!(
                "最近 {total} 条提交中 {night_n} 条处于当地 23:00-06:00（占 {pct}%，按提交时区换算）",
                night_n = night.len()
            ),
            suggestion: "关注团队工作节律：深夜提交常伴随评审缺失与疲劳缺陷，建议调整排班或拆分任务，非阻塞性问题".to_owned(),
        });
    }
    out
}

/// 低质提交信息判定：过短或命中无信息量词表（精确匹配，非前缀）。
fn is_low_quality(summary: &str) -> bool {
    let s = summary
        .trim()
        .trim_end_matches(['.', '!', ':', ';', '-', ' '])
        .to_ascii_lowercase();
    if s.chars().count() < 4 {
        return true;
    }
    matches!(
        s.as_str(),
        "wip"
            | "fix typo"
            | "typo"
            | "fix typos"
            | "tmp"
            | "temp"
            | "test"
            | "testing"
            | "asdf"
            | "qwe"
            | "foo"
            | "bar"
            | "baz"
            | "xxx"
            | "update"
            | "updates"
            | "fix"
            | "fixes"
            | "fixed"
            | "misc"
            | "stuff"
            | "things"
            | "changes"
            | "commit"
            | "oops"
            | "debug"
            | "minor"
            | "cleanup"
    )
}

/// 深夜判定：提交当地小时在 [`NIGHT_HOURS`] 区间（23:00-06:00）。
fn is_night(time: i64, offset_minutes: i32) -> bool {
    let hour = local_hour(time, offset_minutes);
    hour >= NIGHT_HOURS[0] || hour < NIGHT_HOURS[1]
}

/// 提交 hash 前 8 个字符（按字符切片避免 UTF-8 panic）。
fn short_hash(id: &str) -> &str {
    id.get(..id.char_indices().nth(8).map(|(i, _)| i).unwrap_or(id.len()))
        .unwrap_or(id)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{
        is_low_quality, is_night, CommitHabitsChecker, OVERSIZED_CRITICAL, OVERSIZED_WARNING,
    };
    use crate::checker::{Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData};
    use crate::engine::scan::scan;
    use crate::model::finding::{Category, Severity};
    use crate::model::slice_meta::BlobMeta;
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

    fn commit(id: &str, summary: &str, time: i64, offset: i32) -> CommitSummary {
        CommitSummary {
            id: id.into(),
            summary: summary.into(),
            author: "fixture".into(),
            time,
            time_offset: offset,
        }
    }

    /// 批量构造最近提交（id 递推，固定时间：2024-01-01 12:00 UTC 起逐小时，
    /// 均落在非深夜时段，避免干扰比例类规则断言）。
    fn commits(n: usize, summaries: &[&str]) -> Vec<CommitSummary> {
        summaries
            .iter()
            .take(n)
            .enumerate()
            .map(|(i, s)| {
                commit(
                    &format!("{:040}", i + 1),
                    s,
                    1_704_110_400 + i as i64 * 3_600,
                    0,
                )
            })
            .collect()
    }

    /// 构造某提交引入 files 个文件的 blob 列表。
    fn blobs_for(commit_id: &str, files: usize) -> Vec<BlobMeta> {
        (0..files)
            .map(|i| BlobMeta {
                path: format!("f{i}.txt"),
                size: 10,
                first_commit: commit(commit_id, "x", 0, 0),
            })
            .collect()
    }

    fn ctx_with<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        history: Vec<BlobMeta>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        ctx.insert_slice(DataSlice::FullHistory, SliceData::FullHistory(history));
        ctx
    }

    // ===== 命中 =====

    /// 超大提交：>50 文件 Warning；>200 文件 Critical。
    #[test]
    fn 命中_超大提交分级() {
        let snap = RepoSnapshot {
            recent_commits: commits(1, &["big"]),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let mut history = blobs_for(&snap.recent_commits[0].id, OVERSIZED_WARNING + 1);
        history.extend(blobs_for("other", 3));
        let ctx = ctx_with(&snap, &config, history);
        let findings = CommitHabitsChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].id.starts_with("commit-habits.oversized:"));
        assert!(findings[0].suggestion.contains("git add -p"));

        let snap = RepoSnapshot {
            recent_commits: commits(1, &["huge"]),
            ..zero_snapshot()
        };
        let history = blobs_for(&snap.recent_commits[0].id, OVERSIZED_CRITICAL + 1);
        let ctx = ctx_with(&snap, &config, history);
        let findings = CommitHabitsChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
    }

    /// 低质信息占比 >30% → Warning，证据含占比与示例。
    #[test]
    fn 命中_低质信息占比过高() {
        let summaries = [
            "wip",
            "fix typo",
            "x",
            "feat: 新增导出",
            "fix: 边界",
            "docs: 更新",
        ];
        let snap = RepoSnapshot {
            recent_commits: commits(summaries.len(), &summaries),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        let findings = CommitHabitsChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0]
            .id
            .starts_with("commit-habits.low-quality-ratio"));
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].evidence.contains("50%"));
        assert_eq!(findings[0].category, Category::History);
    }

    /// 深夜提交占比 >30% → Info（按提交时区换算）。
    #[test]
    fn 命中_深夜提交占比过高() {
        // 6 条中 3 条为 UTC 23 点（+0 时区即当地 23 点）。
        let recent: Vec<CommitSummary> = (0..6)
            .map(|i| {
                let hour = if i % 2 == 0 { 23 } else { 12 };
                let day = 1 + i / 3;
                commit(
                    &format!("{:040}", i + 1),
                    &format!("commit {i}"),
                    1_704_067_200 + (day - 1) * 86_400 + hour * 3_600,
                    0,
                )
            })
            .collect();
        let snap = RepoSnapshot {
            recent_commits: recent,
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        let findings = CommitHabitsChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("commit-habits.night-ratio"));
        assert_eq!(findings[0].severity, Severity::Info);
    }

    // ===== 不命中 =====

    /// 规范提交（无超大小质低）→ 空。
    #[test]
    fn 不命中_规范提交_返回空() {
        let summaries = [
            "feat: 新增导出",
            "fix: 边界条件",
            "docs: 更新说明",
            "refactor: 拆分模块",
            "test: 补充用例",
        ];
        let snap = RepoSnapshot {
            recent_commits: commits(summaries.len(), &summaries),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert!(CommitHabitsChecker.check(&ctx).is_empty());
    }

    /// 样本不足 5 条时不判定比例类规则。
    #[test]
    fn 不命中_样本不足不判定比例() {
        let snap = RepoSnapshot {
            recent_commits: commits(4, &["wip", "wip", "wip", "wip"]),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert!(CommitHabitsChecker.check(&ctx).is_empty());
    }

    /// 空仓库（无最近提交）→ 空。
    #[test]
    fn 不命中_空仓库_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert!(CommitHabitsChecker.check(&ctx).is_empty());
    }

    // ===== 边界 =====

    /// 恰好 50 文件（严格 > 口径）→ 不判超大。
    #[test]
    fn 边界_恰好50文件不判超大() {
        let snap = RepoSnapshot {
            recent_commits: commits(1, &["edge"]),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let history = blobs_for(&snap.recent_commits[0].id, OVERSIZED_WARNING);
        let ctx = ctx_with(&snap, &config, history);
        assert!(CommitHabitsChecker.check(&ctx).is_empty());
    }

    /// 低质信息函数口径：精确匹配词表，前缀不误伤。
    #[test]
    fn 边界_低质信息函数口径() {
        assert!(is_low_quality("wip"));
        assert!(is_low_quality("WIP."));
        assert!(is_low_quality("fix typo"));
        assert!(is_low_quality("ab"));
        assert!(!is_low_quality("feat: wip 清理"));
        assert!(!is_low_quality("fix: 大文件阈值"));
        assert!(!is_low_quality("feat: 新增导出"));
    }

    /// 深夜函数口径：23 点与 5 点为深夜，6 点与 22 点不是；时区偏移生效。
    #[test]
    fn 边界_深夜函数口径() {
        // 2024-01-01 12:00 UTC
        let noon = 1_704_067_200 + 12 * 3_600;
        assert!(!is_night(noon, 0));
        assert!(is_night(noon, 660), "UTC 12:00 + 11h = 当地 23:00");
        assert!(is_night(noon, -420), "UTC 12:00 - 7h = 当地 05:00");
        let six = 1_704_067_200 + 6 * 3_600;
        assert!(!is_night(six, 0), "06:00 整不算深夜");
    }

    /// 历史切片缺失时比例规则仍可工作，超大提交规则防御性跳过。
    #[test]
    fn 边界_历史切片缺失_防御性跳过超大提交() {
        let snap = RepoSnapshot {
            recent_commits: commits(
                6,
                &[
                    "feat: a",
                    "fix: b",
                    "docs: c",
                    "test: d",
                    "refactor: e",
                    "chore: f",
                ],
            ),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert!(CommitHabitsChecker.check(&ctx).is_empty());
    }

    // ===== 契约 =====

    /// plan 声明幂等：切片恰为 FullHistory。
    #[test]
    fn plan声明幂等_含FullHistory切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = CommitHabitsChecker.plan(&ctx);
        assert_eq!(p1, CommitHabitsChecker.plan(&ctx));
        assert_eq!(p1.slices(), &[DataSlice::FullHistory]);
    }

    /// 同一实例重复 check 等价（纯函数）。
    #[test]
    fn check纯函数_重复等价() {
        let snap = RepoSnapshot {
            recent_commits: commits(6, &["wip", "wip", "wip", "feat: a", "fix: b", "docs: c"]),
            ..zero_snapshot()
        };
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert_eq!(
            CommitHabitsChecker.check(&ctx),
            CommitHabitsChecker.check(&ctx)
        );
    }

    /// 端到端：一次提交 60 个文件 → 超大提交 Warning。
    #[test]
    fn 端到端_超大提交命中() {
        let mut t = TestRepo::init();
        let files: Vec<(String, &[u8])> = (0..60)
            .map(|i| (format!("f{i}.txt"), b"x".as_slice()))
            .collect();
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, c)| (p.as_str(), *c)).collect();
        t.commit(&refs, "mass add");
        t.commit(&[("later.txt", b"y")], "small");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(CommitHabitsChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0]
            .id
            .starts_with("commit-habits.oversized:"));
        assert_eq!(report.scores.history, Some(100 - 8));
    }
}
