//! 扫描编排：开仓 → 快照 → 收集切片计划 → 合并去重 → 定点采集 → 注入上下文 → 逐检查器 check → 聚合 findings → 评分 → 报告。
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::checker::{
    merge_plans, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData,
};
use crate::git::GitRepo;
use crate::ignore::IgnoreRule;
use crate::model::finding::{Category, Finding, Severity};
use crate::model::report::{ScanReport, Scores};
use crate::Result;

/// 历史对象遍历的默认批次大小。
const DEFAULT_BATCH_SIZE: usize = 64;

/// 文件内容切片单文件读取上限（1 MiB）：超过则仅采集元数据。
///
/// 敏感信息等文本扫描无需大文件内容；锁定文件/压缩产物天然超出即跳过。
const FILE_CONTENT_PER_FILE_CAP: u64 = 1_048_576;

/// 文件内容切片全仓读取总预算（32 MiB）：耗尽后后续文件仅采集元数据，
/// 防止 pathological 仓库（海量小文件）读爆内存或耗时失控。
const FILE_CONTENT_TOTAL_BUDGET: u64 = 32 * 1_048_576;

/// 扫描进度事件载荷：引擎在各阶段边界经回调上报给编排层。
///
/// `stage` 为封闭阶段标识：`snapshot` / `workdir` / `history` / `check` / `done`；
/// `done`/`total` 为阶段内进度，`total` 为 0 表示该阶段无确定总量。
/// `check` 阶段每个检查器上报两条事件：进入时 `checker_id` 非空、
/// `finding_count` 为 None（运行中）；完成时 `finding_count` 携带该检查器
/// 产出的 finding 数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanProgress {
    /// 阶段标识（封闭集，见结构体文档）。
    pub stage: &'static str,
    /// 阶段内已完成单元数。
    pub done: usize,
    /// 阶段内单元总数（0 表示无确定总量）。
    pub total: usize,
    /// 检查器 id（仅 `check` 阶段非空）。
    pub checker_id: &'static str,
    /// 检查器产出的 finding 数（仅 check 完成事件携带）。
    pub finding_count: Option<usize>,
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
            cb(ScanProgress {
                stage,
                done,
                total,
                checker_id: "",
                finding_count: None,
            });
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

    // 定点采集：按合并后的计划逐切片落地，未声明的切片保持未就绪。
    for slice in merged.slices() {
        match slice {
            DataSlice::FileContents { pattern } => {
                report_progress("workdir", 0, 0);
                let files = repo.workdir_file_contents(
                    FILE_CONTENT_PER_FILE_CAP,
                    FILE_CONTENT_TOTAL_BUDGET,
                )?;
                let files: Vec<_> = files
                    .into_iter()
                    .filter(|f| !ignore.is_ignored(&f.path) && pattern_matches(pattern, &f.path))
                    .collect();
                ctx.insert_slice(slice.clone(), SliceData::FileContents(files));
                report_progress("workdir", 1, 1);
            }
            DataSlice::FullHistory => {
                let history_cb = on_progress.map(|cb| {
                    |done: usize, total: usize| {
                        cb(ScanProgress {
                            stage: "history",
                            done,
                            total,
                            checker_id: "",
                            finding_count: None,
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
            DataSlice::BranchGraph => {
                let branches = repo.branch_graph()?;
                ctx.insert_slice(slice.clone(), SliceData::BranchGraph(branches));
            }
            DataSlice::DependencyFiles => {
                let deps = repo.dependency_files()?;
                // 依赖目录（node_modules/ 等）内的嵌套清单/锁定文件不是仓库自身的
                // 依赖口径，按忽略规则过滤，避免 vendored 依赖海量误报。
                let deps: Vec<_> = deps
                    .into_iter()
                    .filter(|d| !ignore.is_ignored(&d.path))
                    .collect();
                ctx.insert_slice(slice.clone(), SliceData::DependencyFiles(deps));
            }
            DataSlice::WorkingTree => {}
        }
    }

    // check 阶段：逐检查器纯计算产出 findings，每个检查器进入/完成各上报一次。
    let total_checkers = registry.list().len();
    report_progress("check", 0, total_checkers);
    let mut findings = Vec::new();
    for (i, checker) in registry.list().iter().enumerate() {
        if let Some(cb) = on_progress {
            cb(ScanProgress {
                stage: "check",
                done: i,
                total: total_checkers,
                checker_id: checker.id(),
                finding_count: None,
            });
        }
        let before = findings.len();
        findings.extend(checker.check(&ctx));
        if let Some(cb) = on_progress {
            cb(ScanProgress {
                stage: "check",
                done: i + 1,
                total: total_checkers,
                checker_id: checker.id(),
                finding_count: Some(findings.len() - before),
            });
        }
    }

    // 评分：仅对已注册检查器覆盖的维度计分，未覆盖维度为 None（N/A）。
    let covered: HashSet<Category> = registry.list().iter().map(|c| c.category()).collect();
    let scores = compute_scores(&findings, &covered);

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

/// FileContents 切片模式匹配：`all` 匹配全部；否则按 glob 匹配完整路径或 basename。
fn pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern == "all" {
        return true;
    }
    let basename = path.rsplit('/').next().unwrap_or(path);
    glob_match(pattern, path) || glob_match(pattern, basename)
}

/// 单串 glob 匹配（`*` 任意字符、`?` 单字符；路径分隔符不特殊处理）。
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_p, mut star_t) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star_p = Some(pi);
            star_t = ti;
            pi += 1;
            continue;
        }
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if let Some(sp) = star_p {
            pi = sp + 1;
            star_t += 1;
            ti = star_t;
            continue;
        }
        return false;
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// 按维度计算评分：每个 finding 按严重度扣分（Critical -15 / Warning -8 / Info -3），
/// 各维度独立计算，clamp 到 [0, 100]。
///
/// 覆盖度口径：`covered` 中的维度才计分（从 100 起扣），未覆盖维度为 `None`
/// （前端显示 N/A），杜绝「无检查器的维度恒为 100」的假满分。
fn compute_scores(findings: &[Finding], covered: &HashSet<Category>) -> Scores {
    let mut dims: Vec<(Category, i32)> = Category::ALL
        .iter()
        .map(|c| (*c, if covered.contains(c) { 100 } else { -1 }))
        .collect();

    for f in findings {
        let penalty = match f.severity {
            Severity::Critical => 15,
            Severity::Warning => 8,
            Severity::Info => 3,
        };
        if let Some((_, score)) = dims.iter_mut().find(|(c, _)| *c == f.category) {
            if *score >= 0 {
                *score = (*score - penalty).max(0);
            }
        }
    }

    let score_of = |cat: Category| -> Option<u8> {
        dims.iter()
            .find(|(c, _)| *c == cat)
            .filter(|(_, s)| *s >= 0)
            .map(|(_, s)| *s as u8)
    };
    Scores {
        structure: score_of(Category::Structure),
        history: score_of(Category::History),
        branches: score_of(Category::Branches),
        deps: score_of(Category::Deps),
        security: score_of(Category::Security),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::scan;
    use crate::checker::{
        Checker, CheckerRegistry, DataSlice, ScanContext, ScanPlan, SliceData,
    };
    use crate::model::finding::{Category, Finding, Severity};
    use crate::testutil::TestRepo;

    /// ⑮ 引擎窄链路：空注册表扫描不报错，findings 为空，无覆盖维度总分为 None。
    #[test]
    fn 引擎窄链路_空注册表扫描不报错() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        let registry = CheckerRegistry::new();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
        assert_eq!(report.scores.total(), None, "无覆盖维度时总分应为 N/A");
    }

    /// 进度回调：阶段序列以 snapshot 开头、done 结尾；history 阶段 done 单调不减；
    /// check 阶段逐检查器上报进入/完成两条事件并携带 finding 数。
    #[test]
    fn 进度回调_阶段序列完整且检查器事件含finding数() {
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

        // check 阶段：进入事件 finding_count 为 None，完成事件携带数量
        // （另有一条 checker_id 为空的阶段起始事件，供前端展示 0/N 基线）。
        let check_events: Vec<_> = events
            .iter()
            .filter(|p| p.stage == "check" && !p.checker_id.is_empty())
            .collect();
        assert_eq!(check_events.len(), 2, "单检查器应上报进入+完成两条事件");
        assert_eq!(check_events[0].checker_id, "big-files");
        assert_eq!(check_events[0].finding_count, None);
        assert_eq!(check_events[0].done, 0);
        assert_eq!(check_events[1].checker_id, "big-files");
        assert!(check_events[1].finding_count.is_some());
        assert_eq!(check_events[1].done, 1);
        assert_eq!(check_events[1].total, 1);
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

    /// 覆盖度口径：仅注册 big-files 时 structure 计分，其余四维为 None（N/A）。
    #[test]
    fn 覆盖度_未注册维度评分为None() {
        use std::collections::HashSet;
        let findings = vec![Finding {
            id: "big-files.worktree:a.bin".into(),
            severity: Severity::Critical,
            category: Category::Structure,
            title: "大文件".into(),
            evidence: "5MB".into(),
            suggestion: "加入 gitignore".into(),
        }];
        let covered: HashSet<Category> = [Category::Structure].into_iter().collect();
        let scores = super::compute_scores(&findings, &covered);
        assert_eq!(scores.structure, Some(85));
        assert_eq!(scores.history, None);
        assert_eq!(scores.branches, None);
        assert_eq!(scores.deps, None);
        assert_eq!(scores.security, None);
        assert_eq!(scores.total(), Some(85), "总分只平均已覆盖维度");
    }

    /// 评分按严重度扣分：全维度覆盖时 1 条 Critical 结构维度 finding → structure = 85，其余维度 100。
    #[test]
    fn 评分_按严重度扣分() {
        use std::collections::HashSet;
        let findings = vec![Finding {
            id: "big-files.worktree:a.bin".into(),
            severity: Severity::Critical,
            category: Category::Structure,
            title: "大文件".into(),
            evidence: "5MB".into(),
            suggestion: "加入 gitignore".into(),
        }];
        let covered: HashSet<Category> = Category::ALL.into_iter().collect();
        let scores = super::compute_scores(&findings, &covered);
        assert_eq!(scores.structure, Some(85));
        assert_eq!(scores.history, Some(100));
        assert_eq!(scores.branches, Some(100));
        assert_eq!(scores.deps, Some(100));
        assert_eq!(scores.security, Some(100));
        assert_eq!(scores.total(), Some(100 - 3), "扣分维度拉低总分");
    }

    /// 评分 clamp 到 0：7 条 Critical 同维度 → 100 - 105 → clamp 0。
    #[test]
    fn 评分_clamp到0() {
        use std::collections::HashSet;
        let findings = vec![Finding {
            id: "big-files.worktree:a.bin".into(),
            severity: Severity::Critical,
            category: Category::Structure,
            title: "大文件".into(),
            evidence: "5MB".into(),
            suggestion: "加入 gitignore".into(),
        }; 7];
        let covered: HashSet<Category> = Category::ALL.into_iter().collect();
        let scores = super::compute_scores(&findings, &covered);
        assert_eq!(scores.structure, Some(0));
    }

    /// 未覆盖维度的 finding 不参与计分（防御：理论上不会发生）。
    #[test]
    fn 评分_未覆盖维度的finding不扣分() {
        use std::collections::HashSet;
        let findings = vec![Finding {
            id: "secrets.x:a.txt".into(),
            severity: Severity::Critical,
            category: Category::Security,
            title: "泄露".into(),
            evidence: "e".into(),
            suggestion: "s".into(),
        }];
        let covered: HashSet<Category> = [Category::Structure].into_iter().collect();
        let scores = super::compute_scores(&findings, &covered);
        assert_eq!(scores.security, None, "未覆盖维度不计分");
        assert_eq!(scores.structure, Some(100), "无本维度 finding 保持满分");
    }

    /// FileContents 模式泛化：请求 `*.txt` 时切片仅含匹配文件且携带内容。
    #[test]
    fn 文件内容切片_按模式泛化采集且携带内容() {
        struct PatternChecker;
        impl Checker for PatternChecker {
            fn id(&self) -> &'static str {
                "pattern-probe"
            }
            fn category(&self) -> Category {
                Category::Structure
            }
            fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
                let mut plan = ScanPlan::new();
                plan.push(DataSlice::file_contents("*.txt").unwrap());
                plan
            }
            fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
                let slice = DataSlice::file_contents("*.txt").unwrap();
                let Some(SliceData::FileContents(files)) = ctx.slice(&slice) else {
                    return Vec::new();
                };
                files
                    .iter()
                    .map(|f| Finding {
                        id: format!("pattern-probe:{}", f.path),
                        severity: Severity::Info,
                        category: Category::Structure,
                        title: "seen".into(),
                        evidence: f.content.clone().unwrap_or_default(),
                        suggestion: "none".into(),
                    })
                    .collect()
            }
        }

        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"aaa"), ("b.rs", b"bbb")], "init");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(PatternChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert_eq!(report.findings.len(), 1, "仅 .txt 命中模式采集");
        assert!(report.findings[0].id.contains("a.txt"));
        assert_eq!(report.findings[0].evidence, "aaa", "切片携带文本内容");
    }

    /// 依赖切片过滤忽略目录：node_modules 内嵌套清单不产出结论（默认忽略）。
    #[test]
    fn 依赖切片_忽略目录内嵌套清单不产出结论() {
        let mut t = TestRepo::init();
        t.commit(
            &[("package.json", b"{}"), ("package-lock.json", b"{}")],
            "init",
        );
        std::fs::create_dir_all(t.path().join("node_modules/left-pad")).unwrap();
        std::fs::write(
            t.path().join("node_modules/left-pad/package.json"),
            b"{}",
        )
        .unwrap();
        let mut registry = CheckerRegistry::new();
        registry
            .register(Box::new(crate::checker::DepsHealthChecker))
            .unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(
            report.findings.is_empty(),
            "node_modules 内嵌套清单应被默认忽略: {:?}",
            report.findings.iter().map(|f| &f.id).collect::<Vec<_>>()
        );
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