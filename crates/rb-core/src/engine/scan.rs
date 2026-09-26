//! 扫描编排：开仓 → 快照 → 收集切片计划 → 合并去重 → 定点采集 → 注入上下文 → 逐检查器 check → 聚合 findings → 评分 → 报告。
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::checker::{
    merge_plans, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData,
};
use crate::git::GitRepo;
use crate::model::finding::Severity;
use crate::model::report::{ScanReport, Scores};
use crate::Result;

/// 历史对象遍历的默认批次大小。
const DEFAULT_BATCH_SIZE: usize = 64;

/// 对 `path` 执行完整扫描。
///
/// `cancel_flag` 为 `None` 时不检查取消；为 `Some` 时在批次边界（`history_blob_metas`
/// 每批完成后）检查，用户取消后尽快中止。
pub fn scan(path: &std::path::Path, registry: &CheckerRegistry) -> Result<ScanReport> {
    scan_with_cancel(path, registry, None)
}

/// 带取消检查的完整扫描。
///
/// `cancel_flag` 为 `Some` 时在批次边界检查；用户取消后 `history_blob_metas`
/// 提前返回已采集的部分 blob，扫描继续完成（findings 可能不完整）。
pub fn scan_with_cancel(
    path: &std::path::Path,
    registry: &CheckerRegistry,
    cancel_flag: Option<&AtomicBool>,
) -> Result<ScanReport> {
    let started_at = now_secs();
    let start = Instant::now();

    let repo = GitRepo::open(path)?;
    let snapshot = repo.snapshot()?;

    // plan 阶段：base 上下文收集全部检查器的切片请求。
    let config = ScanConfig::default();
    let mut ctx = ScanContext::base(&snapshot, &config);
    let plans: Vec<_> = registry.list().iter().map(|c| c.plan(&ctx)).collect();
    let merged = merge_plans(&plans);

    // 定点采集：仅落地本特征两切片，其余切片保持未就绪（scan_engine 泛化点）。
    for slice in merged.slices() {
        match slice {
            DataSlice::FileContents { pattern } if pattern == "all" => {
                let metas = repo.workdir_file_metas()?;
                ctx.insert_slice(slice.clone(), SliceData::FileContents(metas));
            }
            DataSlice::FullHistory => {
                let blobs = repo.history_blob_metas(DEFAULT_BATCH_SIZE, cancel_flag)?;
                ctx.insert_slice(slice.clone(), SliceData::FullHistory(blobs));
            }
            _ => {}
        }
    }

    // check 阶段：逐检查器纯计算产出 findings。
    let findings: Vec<_> = registry
        .list()
        .iter()
        .flat_map(|c| c.check(&ctx))
        .collect();

    // 评分：按各维度 finding 严重度扣分。
    let scores = compute_scores(&findings);

    let report = ScanReport {
        repo_path: path.display().to_string(),
        findings,
        scores,
        started_at,
        duration_ms: start.elapsed().as_millis() as u64,
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    Ok(report)
}

/// 按维度计算评分：每个 finding 按严重度扣分（Critical -15 / Warning -8 / Info -3），
/// 各维度独立计算，clamp 到 [0, 100]。无 finding 的维度保持 100。
fn compute_scores(findings: &[crate::model::finding::Finding]) -> Scores {
    let mut structure = 100i32;
    let mut history = 100i32;
    let mut branches = 100i32;
    let mut deps = 100i32;
    let mut security = 100i32;

    for f in findings {
        let penalty = match f.severity {
            Severity::Critical => 15,
            Severity::Warning => 8,
            Severity::Info => 3,
        };
        let dim = match f.category {
            crate::model::finding::Category::Structure => &mut structure,
            crate::model::finding::Category::History => &mut history,
            crate::model::finding::Category::Branches => &mut branches,
            crate::model::finding::Category::Deps => &mut deps,
            crate::model::finding::Category::Security => &mut security,
        };
        *dim = (*dim - penalty).max(0);
    }

    Scores {
        structure: structure as u8,
        history: history as u8,
        branches: branches as u8,
        deps: deps as u8,
        security: security as u8,
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::scan;
    use crate::checker::CheckerRegistry;
    use crate::testutil::TestRepo;

    /// ⑮ 引擎窄链路：空注册表扫描不报错，findings 为空（spec 5.4.3.2 衔接复验）。
    #[test]
    fn 引擎窄链路_空注册表扫描不报错() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        let registry = CheckerRegistry::new();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
        assert_eq!(report.scores.total(), 100);
    }

    /// 评分按严重度扣分：1 条 Critical 结构维度 finding → structure = 85，其余维度 100。
    #[test]
    fn 评分_按严重度扣分() {
        use crate::model::finding::{Category, Finding, Severity};
        let findings = vec![Finding {
            id: "big-files.worktree:a.bin".into(),
            severity: Severity::Critical,
            category: Category::Structure,
            title: "大文件".into(),
            evidence: "5MB".into(),
            suggestion: "加入 gitignore".into(),
        }];
        let scores = super::compute_scores(&findings);
        assert_eq!(scores.structure, 85);
        assert_eq!(scores.history, 100);
        assert_eq!(scores.branches, 100);
        assert_eq!(scores.deps, 100);
        assert_eq!(scores.security, 100);
    }

    /// 评分 clamp 到 0：7 条 Critical 同维度 → 100 - 105 → clamp 0。
    #[test]
    fn 评分_clamp到0() {
        use crate::model::finding::{Category, Finding, Severity};
        let findings = vec![Finding {
            id: "big-files.worktree:a.bin".into(),
            severity: Severity::Critical,
            category: Category::Structure,
            title: "大文件".into(),
            evidence: "5MB".into(),
            suggestion: "加入 gitignore".into(),
        }; 7];
        let scores = super::compute_scores(&findings);
        assert_eq!(scores.structure, 0);
    }
}
