//! 扫描编排：开仓 → 快照 → 收集切片计划 → 合并去重 → 定点采集 → 注入上下文 → 逐检查器 check → 聚合 findings → 评分 → 报告。
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::checker::{
    merge_plans, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData,
};
use crate::git::GitRepo;
use crate::ignore::IgnoreRule;
use crate::model::finding::Severity;
use crate::model::report::{ScanReport, Scores};
use crate::Result;

/// 历史对象遍历的默认批次大小。
const DEFAULT_BATCH_SIZE: usize = 64;

/// 扫描进度事件载荷：引擎在各阶段边界经回调上报给编排层。
///
/// `stage` 为封闭阶段标识：`snapshot` / `workdir` / `history` / `check` / `done`；
/// `done`/`total` 为阶段内进度，`total` 为 0 表示该阶段无确定总量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanProgress {
    /// 阶段标识（封闭集，见结构体文档）。
    pub stage: &'static str,
    /// 阶段内已完成单元数。
    pub done: usize,
    /// 阶段内单元总数（0 表示无确定总量）。
    pub total: usize,
}

/// 对 `path` 执行完整扫描。
///
/// 不检查取消、不上报进度。
pub fn scan(path: &std::path::Path, registry: &CheckerRegistry) -> Result<ScanReport> {
    let config = ScanConfig::default();
    scan_with_progress(path, registry, None, None, &config)
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
    let config = ScanConfig::default();
    scan_with_progress(path, registry, cancel_flag, None, &config)
}

/// 带取消检查、进度回调与自定义配置的完整扫描。
///
/// `cancel_flag` 为 `Some` 时在批次边界检查；用户取消后 `history_blob_metas`
/// 提前返回已采集的部分 blob，扫描继续完成（findings 可能不完整）。
/// `on_progress` 为 `Some` 时在阶段边界（快照、工作区采集、历史采集批次、
/// 逐检查器 check、完成）回调 [`ScanProgress`]，供编排层转发进度事件。
/// `config` 为扫描全局配置（如大文件阈值），由编排层注入。
pub fn scan_with_progress(
    path: &std::path::Path,
    registry: &CheckerRegistry,
    cancel_flag: Option<&AtomicBool>,
    on_progress: Option<&dyn Fn(ScanProgress)>,
    config: &ScanConfig,
) -> Result<ScanReport> {
    let started_at = now_secs();
    let start = Instant::now();

    let report_progress = |stage: &'static str, done: usize, total: usize| {
        if let Some(cb) = on_progress {
            cb(ScanProgress { stage, done, total });
        }
    };

    report_progress("snapshot", 0, 0);
    let repo = GitRepo::open(path)?;
    let snapshot = repo.snapshot()?;
    report_progress("snapshot", 1, 1);

    let ignore = IgnoreRule::load_from(std::path::Path::new(&snapshot.path));

    // plan 阶段：base 上下文收集全部检查器的切片请求。
    let mut ctx = ScanContext::base(&snapshot, config);
    let plans: Vec<_> = registry.list().iter().map(|c| c.plan(&ctx)).collect();
    let merged = merge_plans(&plans);

    // 定点采集：仅落地本特征两切片，其余切片保持未就绪（scan_engine 泛化点）。
    for slice in merged.slices() {
        match slice {
            DataSlice::FileContents { pattern } if pattern == "all" => {
                report_progress("workdir", 0, 0);
                let metas = repo.workdir_file_metas()?;
                let metas: Vec<_> = metas
                    .into_iter()
                    .filter(|m| !ignore.is_ignored(&m.path))
                    .collect();
                ctx.insert_slice(slice.clone(), SliceData::FileContents(metas));
                report_progress("workdir", 1, 1);
            }
            DataSlice::FullHistory => {
                let history_cb = on_progress.map(|cb| {
                    |done: usize, total: usize| {
                        cb(ScanProgress {
                            stage: "history",
                            done,
                            total,
                        })
                    }
                });
                let blobs = repo.history_blob_metas(
                    DEFAULT_BATCH_SIZE,
                    cancel_flag,
                    history_cb
                        .as_ref()
                        .map(|c| c as &dyn Fn(usize, usize)),
                )?;
                let blobs: Vec<_> = blobs
                    .into_iter()
                    .filter(|b| !ignore.is_ignored(&b.path))
                    .collect();
                ctx.insert_slice(slice.clone(), SliceData::FullHistory(blobs));
            }
            _ => {}
        }
    }

    // check 阶段：逐检查器纯计算产出 findings，每个检查器完成即上报进度。
    let total_checkers = registry.list().len();
    report_progress("check", 0, total_checkers);
    let mut findings = Vec::new();
    for (i, checker) in registry.list().iter().enumerate() {
        findings.extend(checker.check(&ctx));
        report_progress("check", i + 1, total_checkers);
    }

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
    report_progress("done", 1, 1);
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

    /// 进度回调：阶段序列以 snapshot 开头、done 结尾；history 阶段 done 单调不减。
    #[test]
    fn 进度回调_阶段序列完整且历史进度单调不减() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        t.commit(&[("b.txt", b"hi")], "second");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(crate::checker::BigFilesChecker)).unwrap();

        let events = std::sync::Mutex::new(Vec::new());
        let config = crate::checker::ScanConfig::default();
        super::scan_with_progress(t.path(), &registry, None, Some(&|p| events.lock().unwrap().push(p)), &config)
            .unwrap();
        let events = events.into_inner().unwrap();

        let stages: Vec<&str> = events.iter().map(|p| p.stage).collect();
        assert_eq!(stages.first(), Some(&"snapshot"), "首个阶段应为 snapshot");
        assert_eq!(stages.last(), Some(&"done"), "末个阶段应为 done");
        assert!(stages.contains(&"workdir"));
        assert!(stages.contains(&"history"));
        assert!(stages.contains(&"check"));

        // 历史阶段 done 单调不减，且最终 done == total。
        let history: Vec<_> = events.iter().filter(|p| p.stage == "history").collect();
        for w in history.windows(2) {
            assert!(w[0].done <= w[1].done, "history done 应单调不减");
        }
        let last_history = history.last().unwrap();
        assert_eq!(last_history.done, last_history.total);

        // check 阶段最终 done == 检查器总数（此处仅注册 big-files 一个）。
        let check_last = events.iter().rev().find(|p| p.stage == "check").unwrap();
        assert_eq!(check_last.done, 1);
        assert_eq!(check_last.total, 1);
    }

    /// 旧 API scan/scan_with_cancel 委托后行为不变（不上报进度也能完成扫描）。
    #[test]
    fn 旧入口委托_无回调时扫描正常() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(crate::checker::BigFilesChecker)).unwrap();
        let report = super::scan_with_cancel(t.path(), &registry, None).unwrap();
        assert!(report.findings.len() <= 2);
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

    /// `.repobalance-ignore` 规则生效：被忽略的大文件不产出 finding。
    #[test]
    fn 忽略规则_被忽略的大文件不产出finding() {
        let mut t = TestRepo::init();
        t.commit(&[("big.bin", &vec![0u8; 2_000_000])], "add big");
        std::fs::write(
            t.path().join(".repobalance-ignore"),
            "*.bin\n",
        )
        .unwrap();
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(crate::checker::BigFilesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(
            report.findings.is_empty(),
            "被 .repobalance-ignore 忽略的大文件不应产出 finding"
        );
    }

    /// 否定规则：`*.bin` 忽略但 `!keep.bin` 取消忽略，后者仍产出 finding。
    #[test]
    fn 忽略规则_否定规则取消忽略() {
        let mut t = TestRepo::init();
        t.commit(&[("keep.bin", &vec![0u8; 2_000_000])], "add keep");
        std::fs::write(
            t.path().join(".repobalance-ignore"),
            "*.bin\n!keep.bin\n",
        )
        .unwrap();
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(crate::checker::BigFilesChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0].id.contains("keep.bin"));
    }
}
