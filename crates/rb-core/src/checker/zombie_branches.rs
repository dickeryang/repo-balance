//! 僵尸分支检查器（Branches 维度）：已合并未删除、超 N 天未活跃、
//! 无远端追踪三类规则。
//!
//! 纯计算：消费 `BranchGraph` 切片（引擎采集分支拓扑），不触碰 git。
//!
//! # 「未活跃」参考基准
//! 检查器为纯函数（同输入同输出），不读系统时钟：以仓库内**最新分支 tip
//! 时间**为参考基准计算各分支年龄。活跃仓库中该基准≈当前时间；归档仓库
//! （最新提交本身很久以前）不会误报全员僵尸。

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};
use crate::model::slice_meta::BranchMeta;
use crate::timefmt::format_utc;

/// 僵尸分支检查器：无状态、纯计算。
pub struct ZombieBranchesChecker;

/// 每日秒数。
const DAY_SECS: i64 = 86_400;

impl Checker for ZombieBranchesChecker {
    fn id(&self) -> &'static str {
        "zombie-branches"
    }

    fn category(&self) -> Category {
        Category::Branches
    }

    /// 幂等声明分支拓扑切片。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::BranchGraph);
        plan
    }

    /// 规则（优先级从高到低，每个分支至多一条结论）：
    /// 1. 已合并未删除 → Warning（可安全删除）；
    /// 2. 超 `zombie_branch_days` 天未活跃（未合并）→ Warning；
    /// 3. 无远端追踪（且不满足上述两条）→ Info。
    ///
    /// 当前 HEAD 分支不参与判定。切片未就绪时防御性返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let Some(SliceData::BranchGraph(branches)) = ctx.slice(&DataSlice::BranchGraph) else {
            return Vec::new();
        };
        if branches.is_empty() {
            return Vec::new();
        }
        let reference = branches
            .iter()
            .map(|b| b.tip_commit.time)
            .max()
            .unwrap_or_default();
        let threshold_secs = ctx
            .config()
            .zombie_branch_days
            .saturating_mul(DAY_SECS as u64) as i64;

        let mut findings = Vec::new();
        for b in branches {
            if b.is_current {
                continue; // 当前分支即活跃分支，不判定
            }
            if b.is_merged {
                findings.push(merged_finding(b));
                continue;
            }
            let age = reference - b.tip_commit.time;
            if age > threshold_secs {
                findings.push(stale_finding(b, age / DAY_SECS));
                continue;
            }
            if !b.has_upstream {
                findings.push(no_upstream_finding(b));
            }
        }
        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 分支证据公共部分。
fn branch_evidence(b: &BranchMeta) -> String {
    format!(
        "分支: {}\ntip 提交: {} {}\ntip 时间: {}\n已合并: {} · 远端追踪: {}",
        b.name,
        short_hash(&b.tip_commit.id),
        b.tip_commit.summary,
        format_utc(b.tip_commit.time),
        if b.is_merged { "是" } else { "否" },
        if b.has_upstream { "有" } else { "无" },
    )
}

/// 已合并未删除 → Warning。
fn merged_finding(b: &BranchMeta) -> Finding {
    Finding {
        id: format!("zombie-branches.merged:{}", b.name),
        severity: Severity::Warning,
        category: Category::Branches,
        title: format!("已合并未删除的分支: {}", b.name),
        evidence: branch_evidence(b),
        suggestion: format!(
            "该分支已合并进当前 HEAD，可安全删除：git branch -d {}；如需保留备份请先 git push origin {}",
            b.name, b.name
        ),
    }
}

/// 超期未活跃 → Warning。
fn stale_finding(b: &BranchMeta, age_days: i64) -> Finding {
    Finding {
        id: format!("zombie-branches.stale:{}", b.name),
        severity: Severity::Warning,
        category: Category::Branches,
        title: format!("超过 90 天未活跃的分支: {}", b.name),
        evidence: format!(
            "{}\n未活跃: 约 {age_days} 天（以仓库最新分支 tip 为基准）",
            branch_evidence(b)
        ),
        suggestion: format!(
            "确认分支存废：已废弃执行 git branch -D {}；仍需保留请 git push origin {} 建立远端备份，避免单点丢失",
            b.name, b.name
        ),
    }
}

/// 无远端追踪 → Info。
fn no_upstream_finding(b: &BranchMeta) -> Finding {
    Finding {
        id: format!("zombie-branches.no-upstream:{}", b.name),
        severity: Severity::Info,
        category: Category::Branches,
        title: format!("无远端追踪的本地分支: {}", b.name),
        evidence: branch_evidence(b),
        suggestion: format!(
            "本地分支无远端追踪，仅存在于本机：git push -u origin {} 建立追踪；确认为临时分支可在清理时删除",
            b.name
        ),
    }
}

/// 提交 hash 前 8 个字符（按字符切片避免 UTF-8 panic）。
fn short_hash(id: &str) -> &str {
    id.get(..id.char_indices().nth(8).map(|(i, _)| i).unwrap_or(id.len()))
        .unwrap_or(id)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::ZombieBranchesChecker;
    use crate::checker::{Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData};
    use crate::engine::scan::scan;
    use crate::model::finding::{Category, Severity};
    use crate::model::slice_meta::BranchMeta;
    use crate::model::snapshot::{CommitSummary, RepoSnapshot};
    use crate::testutil::TestRepo;

    /// 参考基准时间（fixtures 起点 + 10 天）。
    const NOW: i64 = 1_700_000_000 + 10 * 86_400;

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

    fn commit(id: &str, time: i64) -> CommitSummary {
        CommitSummary {
            id: id.into(),
            summary: format!("commit {id}"),
            author: "fixture".into(),
            time,
            time_offset: 0,
        }
    }

    fn branch(
        name: &str,
        time: i64,
        is_merged: bool,
        has_upstream: bool,
        is_current: bool,
    ) -> BranchMeta {
        BranchMeta {
            name: name.into(),
            tip_commit: commit(&format!("{name}-tip"), time),
            is_merged,
            has_upstream,
            is_current,
        }
    }

    fn ctx_with<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        branches: Vec<BranchMeta>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        ctx.insert_slice(DataSlice::BranchGraph, SliceData::BranchGraph(branches));
        ctx
    }

    // ===== 命中 =====

    /// 已合并未删除 → Warning，建议含 git branch -d。
    #[test]
    fn 命中_已合并未删除_产出Warning() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("feature", NOW - 3_600, true, false, false),
            ],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].id.starts_with("zombie-branches.merged:feature"));
        assert!(findings[0].suggestion.contains("git branch -d feature"));
        assert_eq!(findings[0].category, Category::Branches);
    }

    /// 超 90 天未活跃（未合并）→ Warning。
    #[test]
    fn 命中_超期未活跃_产出Warning() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("old", NOW - 200 * 86_400, false, true, false),
            ],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("zombie-branches.stale:old"));
        assert!(findings[0].evidence.contains("200 天"));
    }

    /// 活跃但无远端追踪 → Info。
    #[test]
    fn 命中_无远端追踪_产出Info() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![branch("wip", NOW - 3_600, false, false, false)],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
        assert!(findings[0]
            .id
            .starts_with("zombie-branches.no-upstream:wip"));
        assert!(findings[0].suggestion.contains("git push -u origin wip"));
    }

    /// 阈值可配：注入 30 天阈值后 45 天未活跃命中。
    #[test]
    fn 阈值可配_注入30天后45天未活跃命中() {
        let snap = zero_snapshot();
        let config = ScanConfig {
            big_file_threshold: 1_048_576,
            zombie_branch_days: 30,
            ..Default::default()
        };
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("mid", NOW - 45 * 86_400, false, true, false),
            ],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("zombie-branches.stale:mid"));
    }

    // ===== 不命中 =====

    /// 全部健康分支（活跃 + 有远端 + 未合并）→ 空。
    #[test]
    fn 不命中_健康分支_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("dev", NOW - 86_400, false, true, false),
            ],
        );
        assert!(ZombieBranchesChecker.check(&ctx).is_empty());
    }

    /// 当前 HEAD 分支即使已合并/超期也不判定。
    #[test]
    fn 不命中_当前分支跳过() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![branch("main", NOW - 400 * 86_400, true, false, true)],
        );
        assert!(ZombieBranchesChecker.check(&ctx).is_empty());
    }

    /// 空仓库（无分支）→ 空。
    #[test]
    fn 不命中_无分支_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert!(ZombieBranchesChecker.check(&ctx).is_empty());
    }

    // ===== 边界 =====

    /// 恰好 90 天（严格 > 口径）→ 不判 stale。
    #[test]
    fn 边界_恰好90天不判僵尸() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("edge", NOW - 90 * 86_400, false, true, false),
            ],
        );
        assert!(ZombieBranchesChecker.check(&ctx).is_empty());
    }

    /// 超过 1 秒即判 stale（阈值边界敏感）。
    #[test]
    fn 边界_超过阈值1秒判僵尸() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                branch("main", NOW, false, true, true),
                branch("edge", NOW - 90 * 86_400 - 1, false, true, false),
            ],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("zombie-branches.stale:edge"));
    }

    /// 已合并优先于超期：合并 + 超期只产出 merged 一条。
    #[test]
    fn 边界_已合并优先于超期() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![branch("legacy", NOW - 400 * 86_400, true, false, false)],
        );
        let findings = ZombieBranchesChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("zombie-branches.merged:"));
    }

    // ===== 契约 =====

    /// plan 声明幂等：切片恰为 BranchGraph。
    #[test]
    fn plan声明幂等_含BranchGraph切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = ZombieBranchesChecker.plan(&ctx);
        assert_eq!(p1, ZombieBranchesChecker.plan(&ctx));
        assert_eq!(p1.slices(), &[DataSlice::BranchGraph]);
    }

    /// 同一实例重复 check 等价（纯函数）；切片缺失时防御性返回空。
    #[test]
    fn check纯函数_重复等价_切片缺失返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let empty_ctx = ScanContext::base(&snap, &config);
        assert!(ZombieBranchesChecker.check(&empty_ctx).is_empty());

        let ctx = ctx_with(
            &snap,
            &config,
            vec![branch("wip", NOW, false, false, false)],
        );
        assert_eq!(
            ZombieBranchesChecker.check(&ctx),
            ZombieBranchesChecker.check(&ctx)
        );
    }

    /// 端到端：祖先分支（已合并）+ 有新提交的分支（未合并无远端）→ 两类结论。
    #[test]
    fn 端到端_合并与无远端分支均产出结论() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "c1");
        t.branch("feature"); // tip = c1，之后 main 前进 → 已合并
        t.commit(&[("b.txt", b"hi")], "c2 on main");
        t.branch("wip");
        t.checkout("wip");
        t.commit(&[("c.txt", b"hi")], "c3 on wip");
        t.checkout("main");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ZombieBranchesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let ids: Vec<&str> = report.findings.iter().map(|f| f.id.as_str()).collect();
        assert!(
            ids.iter()
                .any(|id| id.starts_with("zombie-branches.merged:feature")),
            "已合并分支应有结论: {ids:?}"
        );
        assert!(
            ids.iter()
                .any(|id| id.starts_with("zombie-branches.no-upstream:wip")),
            "无远端追踪分支应有结论: {ids:?}"
        );
        assert!(
            !ids.iter().any(|id| id.contains(":main")),
            "当前 HEAD 分支不应有结论"
        );
    }

    /// 端到端：单分支健康仓库 → 无结论。
    #[test]
    fn 端到端_单分支仓库无结论() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ZombieBranchesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }
}
