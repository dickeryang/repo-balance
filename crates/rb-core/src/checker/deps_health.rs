//! 依赖健康检查器（Deps 维度）：结构检查 + 已知漏洞匹配。
//!
//! 结构规则：锁定文件缺失、锁定文件未提交、锁定文件被删（manifest 与 lock 不同步）、
//! 同生态多包管理器混用。
//!
//! 漏洞规则：解析各依赖文件的（包名, 版本），与注入的离线漏洞库索引做版本匹配。
//! **未接入漏洞库时该规则不产出任何结论**（`ctx.vuln_index()` 为 None），
//! 维度得分与接入前完全一致——即这项能力是「可选增强」而非「必备依赖」。
//!
//! 纯计算：消费 `DependencyFiles` 切片（元数据）与 `DependencyFileContents` 切片（内容）。

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};
use crate::model::slice_meta::{DepFileKind, DepFileMeta, Ecosystem};
use crate::model::vuln::{Advisory, VulnEcosystem};
use crate::vuln::manifest::{parse_dep_file, ParsedDep};

/// 依赖健康检查器：无状态、纯计算。
pub struct DepsHealthChecker;

impl Checker for DepsHealthChecker {
    fn id(&self) -> &'static str {
        "deps-health"
    }

    fn category(&self) -> Category {
        Category::Deps
    }

    /// 声明依赖清单切片；仅在接入漏洞库时追加内容切片。
    ///
    /// 内容切片的采集有 I/O 成本，未接入漏洞库时不会有人消费它，故按需声明。
    fn plan(&self, ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::DependencyFiles);
        if ctx.vuln_index().is_some() {
            plan.push(DataSlice::DependencyFileContents);
        }
        plan
    }

    /// 结构检查四类规则 + 漏洞匹配；切片未就绪时防御性返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let Some(SliceData::DependencyFiles(deps)) = ctx.slice(&DataSlice::DependencyFiles) else {
            return Vec::new();
        };

        let mut findings = Vec::new();
        for eco in Ecosystem::ALL {
            let manifests: Vec<&DepFileMeta> = deps
                .iter()
                .filter(|d| d.ecosystem == eco && d.kind == DepFileKind::Manifest)
                .collect();
            let locks: Vec<&DepFileMeta> = deps
                .iter()
                .filter(|d| d.ecosystem == eco && d.kind == DepFileKind::Lock)
                .collect();

            // 规则一：清单存在但锁定文件整体缺失。
            if lock_required(eco, &manifests) && locks.is_empty() {
                if let Some(m) = manifests.first() {
                    findings.push(missing_lock_finding(eco, m));
                }
            }

            // 规则二/三：锁定文件的 git 跟踪状态与工作区存在性不一致。
            for l in &locks {
                if l.in_workdir && !l.tracked {
                    findings.push(lock_untracked_finding(l));
                } else if l.tracked && !l.in_workdir {
                    findings.push(lock_missing_finding(l));
                }
            }

            // 规则四：同生态多包管理器混用。
            match eco {
                Ecosystem::JavaScript if locks.len() > 1 => {
                    findings.push(mixed_finding(
                        eco,
                        &locks.iter().map(|l| l.path.clone()).collect::<Vec<_>>(),
                        "lockfile",
                    ));
                }
                Ecosystem::Python if manifests.len() > 1 => {
                    findings.push(mixed_finding(
                        eco,
                        &manifests.iter().map(|m| m.path.clone()).collect::<Vec<_>>(),
                        "manifest",
                    ));
                }
                _ => {}
            }
        }

        // 规则五：已知漏洞匹配（需漏洞库 + 依赖文件内容，缺一不产出结论）。
        findings.extend(vuln_findings(ctx));

        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 漏洞匹配结论：逐个依赖文件解析（包名, 版本），命中索引即产出一条 finding。
///
/// 无漏洞库或无内容切片时返回空——这项能力是可选增强，不得影响结构检查结论。
fn vuln_findings(ctx: &ScanContext) -> Vec<Finding> {
    let (Some(index), Some(SliceData::DependencyFileContents(files))) = (
        ctx.vuln_index(),
        ctx.slice(&DataSlice::DependencyFileContents),
    ) else {
        return Vec::new();
    };

    // 去重口径是「公告 + 包名」，不含版本：
    // - 同一依赖常同时出现在清单与锁定文件（package.json + package-lock.json），
    //   版本写法还不同（"^4.17.20" vs "4.17.20"），按版本去重会漏掉；
    // - 锁定文件里同一包也可能有多版本共存（传递依赖），逐版本产出会产生
    //   重复 finding id，污染历史对比与界面展开状态。
    // 因此按「公告 + 包名」只产出一个结论，版本优先采信锁定文件（实际安装版本）。
    let mut hits: HashMap<String, VulnHit<'_>> = HashMap::new();
    for file in files {
        let Some(eco) = VulnEcosystem::from_ecosystem(file.ecosystem) else {
            continue; // 该生态未收录索引（Ruby / PHP）
        };
        for dep in parse_dep_file(&file.path, file.content.as_deref(), file.ecosystem) {
            for advisory in index.matches(eco, &dep.name, &dep.version) {
                let key = format!("{}|{}", advisory.id, dep.name);
                let candidate = VulnHit {
                    advisory,
                    dep: dep.clone(),
                    path: &file.path,
                    is_lock: file.kind == DepFileKind::Lock,
                };
                match hits.entry(key) {
                    Entry::Vacant(e) => {
                        e.insert(candidate);
                    }
                    Entry::Occupied(mut e) => {
                        if candidate.is_lock && !e.get().is_lock {
                            e.insert(candidate);
                        }
                    }
                }
            }
        }
    }

    let mut findings: Vec<Finding> = hits
        .values()
        .map(|h| vuln_finding(h.path, &h.dep, h.advisory))
        .collect();
    findings.sort_by(|a, b| a.id.cmp(&b.id));
    findings
}

/// 漏洞匹配的中间结果（用于跨文件去重）。
struct VulnHit<'a> {
    advisory: &'a Advisory,
    dep: ParsedDep,
    path: &'a str,
    /// 是否来自锁定文件（去重时优先）。
    is_lock: bool,
}

/// 单条漏洞结论（id 含包名与公告 id，便于去重与历史对比）。
fn vuln_finding(path: &str, dep: &ParsedDep, advisory: &Advisory) -> Finding {
    let severity = advisory.severity();
    let ranges: Vec<String> = advisory.ranges.iter().map(|r| r.describe()).collect();
    let range_text = if ranges.is_empty() {
        "全部版本".to_owned()
    } else {
        ranges.join(" 或 ")
    };
    let summary = if advisory.summary.is_empty() {
        "（上游公告未提供摘要）"
    } else {
        &advisory.summary
    };
    Finding {
        id: format!("deps-health.vuln:{}:{}", advisory.id, dep.name),
        severity,
        category: Category::Deps,
        title: format!(
            "依赖存在已知漏洞: {} {}（{}）",
            dep.name, dep.version, advisory.id
        ),
        evidence: format!(
            "文件: {}\n包: {} {}\n公告: {}\n受影响区间: {}\n摘要: {}",
            path, dep.name, dep.version, advisory.id, range_text, summary
        ),
        suggestion: format!(
            "升级 {} 到不受影响版本（受影响区间 {}），随后重新生成锁定文件并提交；\
             若暂时无法升级，请评估该漏洞在本项目中的实际可达性并记录豁免理由",
            dep.name, range_text
        ),
    }
}

/// 该生态是否强制要求锁定文件。
///
/// - Java 无强制锁定（Maven 无 lock 概念，Gradle 可选）；
/// - Python 仅当存在 pyproject.toml / Pipfile 时要求（requirements.txt 无锁定标准）；
/// - 其余生态有清单即要求。
fn lock_required(eco: Ecosystem, manifests: &[&DepFileMeta]) -> bool {
    if manifests.is_empty() {
        return false;
    }
    match eco {
        Ecosystem::Java => false,
        Ecosystem::Python => manifests
            .iter()
            .any(|m| file_name(&m.path) != "requirements.txt"),
        _ => true,
    }
}

/// 规则一：锁定文件缺失 → Warning。
fn missing_lock_finding(eco: Ecosystem, manifest: &DepFileMeta) -> Finding {
    let hint = match eco {
        Ecosystem::Rust => "执行 cargo generate-lockfile 生成并提交 Cargo.lock",
        Ecosystem::JavaScript => {
            "执行 npm install 生成 package-lock.json 并提交（或统一使用 pnpm/yarn 的锁定文件）"
        }
        Ecosystem::Python => "执行 poetry lock 或 uv lock 生成锁定文件并提交",
        Ecosystem::Go => "执行 go mod tidy 生成 go.sum 并提交",
        Ecosystem::Ruby => "执行 bundle install 生成 Gemfile.lock 并提交",
        Ecosystem::Php => "执行 composer install 生成 composer.lock 并提交",
        Ecosystem::Java => "如使用 Gradle 依赖锁定，生成 gradle.lockfile 并提交",
    };
    Finding {
        id: format!("deps-health.missing-lock:{}", manifest.path),
        severity: Severity::Warning,
        category: Category::Deps,
        title: format!("缺少锁定文件: {}（{}）", manifest.path, eco_label(eco)),
        evidence: format!(
            "清单: {}\n生态: {}\n锁定文件: 未找到（工作区与 git 均无）",
            manifest.path,
            eco_label(eco)
        ),
        suggestion: hint.to_owned(),
    }
}

/// 规则二：锁定文件存在于工作区但未提交 → Warning。
fn lock_untracked_finding(lock: &DepFileMeta) -> Finding {
    Finding {
        id: format!("deps-health.lock-untracked:{}", lock.path),
        severity: Severity::Warning,
        category: Category::Deps,
        title: format!("锁定文件未提交到版本控制: {}", lock.path),
        evidence: format!(
            "锁定文件: {}\n状态: 工作区存在但未被 git 跟踪\n说明: 未提交的锁定文件无法保证协作者/CI 安装同一版本",
            lock.path
        ),
        suggestion: format!("执行 git add {} 并提交；如为有意忽略请在 .repobalance-ignore 中显式声明", lock.path),
    }
}

/// 规则三：锁定文件被 git 跟踪但工作区缺失（manifest 与 lock 不同步）→ Info。
fn lock_missing_finding(lock: &DepFileMeta) -> Finding {
    Finding {
        id: format!("deps-health.lock-missing:{}", lock.path),
        severity: Severity::Info,
        category: Category::Deps,
        title: format!("锁定文件在仓库中但工作区缺失: {}", lock.path),
        evidence: format!(
            "锁定文件: {}\n状态: 已被 git 跟踪但工作区不存在\n说明: 可能是本地误删或 .gitignore 误忽略，会导致依赖安装结果不确定",
            lock.path
        ),
        suggestion: format!("执行 git checkout -- {} 恢复；若确认不再需要请用 git rm --cached {} 同步移除", lock.path, lock.path),
    }
}

/// 规则四：同生态多包管理器混用 → Info。
fn mixed_finding(eco: Ecosystem, paths: &[String], kind_label: &str) -> Finding {
    Finding {
        id: format!("deps-health.mixed-{kind_label}s:{}", eco_key(eco)),
        severity: Severity::Info,
        category: Category::Deps,
        title: format!("同生态多个包管理器混用: {}", eco_label(eco)),
        evidence: format!(
            "生态: {}\n{kind_label}: {}\n说明: 多个管理器会产生多份锁定文件，版本口径可能互相覆盖",
            eco_label(eco),
            paths.join("、")
        ),
        suggestion: "统一为一个包管理器，删除其余锁定文件并更新 CI 安装命令".to_owned(),
    }
}

/// 生态键名（finding id 中段）。
fn eco_key(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Rust => "rust",
        Ecosystem::JavaScript => "javascript",
        Ecosystem::Python => "python",
        Ecosystem::Go => "go",
        Ecosystem::Ruby => "ruby",
        Ecosystem::Php => "php",
        Ecosystem::Java => "java",
    }
}

/// 生态中文标签。
fn eco_label(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Rust => "Rust",
        Ecosystem::JavaScript => "JavaScript",
        Ecosystem::Python => "Python",
        Ecosystem::Go => "Go",
        Ecosystem::Ruby => "Ruby",
        Ecosystem::Php => "PHP",
        Ecosystem::Java => "Java",
    }
}

/// 取路径末段（文件名）。
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::DepsHealthChecker;
    use crate::checker::{Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData};
    use crate::engine::scan::scan;
    use crate::model::finding::{Category, Finding, Severity};
    use crate::model::slice_meta::{DepFileKind, DepFileMeta, Ecosystem};
    use crate::model::snapshot::RepoSnapshot;
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

    fn dep(
        path: &str,
        eco: Ecosystem,
        kind: DepFileKind,
        tracked: bool,
        in_workdir: bool,
    ) -> DepFileMeta {
        DepFileMeta {
            path: path.into(),
            ecosystem: eco,
            kind,
            tracked,
            in_workdir,
        }
    }

    fn ctx_with<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        deps: Vec<DepFileMeta>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        ctx.insert_slice(DataSlice::DependencyFiles, SliceData::DependencyFiles(deps));
        ctx
    }

    fn ids(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.id.as_str()).collect()
    }

    // ===== 命中 =====

    /// Cargo.toml 存在但无 Cargo.lock → Warning。
    #[test]
    fn 命中_缺少锁定文件_产出Warning() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![dep(
                "Cargo.toml",
                Ecosystem::Rust,
                DepFileKind::Manifest,
                true,
                true,
            )],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0]
            .id
            .starts_with("deps-health.missing-lock:Cargo.toml"));
        assert!(findings[0].suggestion.contains("cargo generate-lockfile"));
    }

    /// 锁定文件工作区存在但未跟踪 → Warning。
    #[test]
    fn 命中_锁定文件未提交_产出Warning() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "package.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "package-lock.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Lock,
                    false,
                    true,
                ),
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("deps-health.lock-untracked:"));
        assert!(findings[0].suggestion.contains("git add package-lock.json"));
    }

    /// 锁定文件被跟踪但工作区缺失 → Info。
    #[test]
    fn 命中_锁定文件被删_产出Info() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "Cargo.toml",
                    Ecosystem::Rust,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "Cargo.lock",
                    Ecosystem::Rust,
                    DepFileKind::Lock,
                    true,
                    false,
                ),
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
        assert!(findings[0].id.starts_with("deps-health.lock-missing:"));
    }

    /// JS 多个锁定文件混用 → Info。
    #[test]
    fn 命中_js多锁定文件混用_产出Info() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "package.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "package-lock.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
                dep(
                    "yarn.lock",
                    Ecosystem::JavaScript,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
        assert!(findings[0]
            .id
            .starts_with("deps-health.mixed-lockfiles:javascript"));
        assert!(findings[0].evidence.contains("yarn.lock"));
    }

    /// Python 多个清单文件混用 → Info。
    #[test]
    fn 命中_py多清单混用_产出Info() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "pyproject.toml",
                    Ecosystem::Python,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "requirements.txt",
                    Ecosystem::Python,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "poetry.lock",
                    Ecosystem::Python,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0]
            .id
            .starts_with("deps-health.mixed-manifests:python"));
    }

    /// pyproject.toml 无锁定文件 → Warning（Python 强清单口径）。
    #[test]
    fn 命中_pyproject无锁_产出Warning() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![dep(
                "pyproject.toml",
                Ecosystem::Python,
                DepFileKind::Manifest,
                true,
                true,
            )],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0]
            .id
            .starts_with("deps-health.missing-lock:pyproject.toml"));
    }

    // ===== 不命中 =====

    /// 完整锁定（清单 + 锁均提交）→ 空。
    #[test]
    fn 不命中_完整锁定_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "Cargo.toml",
                    Ecosystem::Rust,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep("Cargo.lock", Ecosystem::Rust, DepFileKind::Lock, true, true),
                dep(
                    "package.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "package-lock.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
            ],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// requirements.txt 单独存在（pip 无锁定标准）→ 空。
    #[test]
    fn 不命中_requirements单独_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![dep(
                "requirements.txt",
                Ecosystem::Python,
                DepFileKind::Manifest,
                true,
                true,
            )],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// Java 无强制锁定 → 空。
    #[test]
    fn 不命中_java无锁定要求_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![dep(
                "pom.xml",
                Ecosystem::Java,
                DepFileKind::Manifest,
                true,
                true,
            )],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// 无任何依赖文件 → 空。
    #[test]
    fn 不命中_无依赖文件_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(&snap, &config, vec![]);
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    // ===== 边界 =====

    /// pyproject + poetry.lock 齐备 → 无缺失结论（仅结构检查不判版本同步）。
    #[test]
    fn 边界_pyproject有锁_无缺失结论() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep(
                    "pyproject.toml",
                    Ecosystem::Python,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "poetry.lock",
                    Ecosystem::Python,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
            ],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// 未跟踪的清单 + 未跟踪的锁：不重复报 missing-lock（锁已存在）。
    #[test]
    fn 边界_锁存在但未跟踪_只报未提交() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with(
            &snap,
            &config,
            vec![
                dep("go.mod", Ecosystem::Go, DepFileKind::Manifest, false, true),
                dep("go.sum", Ecosystem::Go, DepFileKind::Lock, false, true),
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(ids(&findings), vec!["deps-health.lock-untracked:go.sum"]);
    }

    // ===== 契约 =====

    /// plan 声明幂等：切片恰为 DependencyFiles。
    #[test]
    fn plan声明幂等_含DependencyFiles切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = DepsHealthChecker.plan(&ctx);
        assert_eq!(p1, DepsHealthChecker.plan(&ctx));
        assert_eq!(p1.slices(), &[DataSlice::DependencyFiles]);
    }

    /// 同一实例重复 check 等价（纯函数）；切片缺失时防御性返回空。
    #[test]
    fn check纯函数_重复等价_切片缺失返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let empty_ctx = ScanContext::base(&snap, &config);
        assert!(DepsHealthChecker.check(&empty_ctx).is_empty());

        let ctx = ctx_with(
            &snap,
            &config,
            vec![dep(
                "Cargo.toml",
                Ecosystem::Rust,
                DepFileKind::Manifest,
                true,
                true,
            )],
        );
        assert_eq!(DepsHealthChecker.check(&ctx), DepsHealthChecker.check(&ctx));
    }

    /// 端到端：Cargo.toml 已提交但无 Cargo.lock → missing-lock Warning。
    #[test]
    fn 端到端_缺少锁定文件命中() {
        let mut t = TestRepo::init();
        t.commit(&[("Cargo.toml", b"[package]\nname = \"x\"\n")], "init");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(DepsHealthChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0]
            .id
            .starts_with("deps-health.missing-lock:Cargo.toml"));
        assert_eq!(report.findings[0].category, Category::Deps);
        assert_eq!(report.scores.deps, Some(100 - 8));
    }

    /// 端到端：清单 + 锁均提交 → 无结论。
    #[test]
    fn 端到端_完整锁定无结论() {
        let mut t = TestRepo::init();
        t.commit(
            &[
                ("Cargo.toml", b"[package]\nname = \"x\"\n"),
                ("Cargo.lock", b"# lock\n"),
            ],
            "init",
        );
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(DepsHealthChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }

    // ===== 规则五：已知漏洞匹配 =====

    use crate::model::slice_meta::DepFileContent;
    use crate::model::vuln::{Advisory, VulnEcosystem, VulnIndex, VulnRange};
    use std::sync::Arc;

    /// 构造一个含漏洞库与依赖文件内容的上下文。
    fn vuln_ctx<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        deps: Vec<DepFileMeta>,
        files: Vec<DepFileContent>,
    ) -> ScanContext<'a> {
        let mut ctx = ctx_with(snapshot, config, deps);
        ctx.insert_slice(
            DataSlice::DependencyFileContents,
            SliceData::DependencyFileContents(files),
        );
        ctx
    }

    fn adv(id: &str, eco: VulnEcosystem, pkg: &str, introduced: &str, fixed: &str) -> Advisory {
        Advisory {
            id: id.into(),
            eco,
            pkg: pkg.into(),
            ranges: vec![VulnRange {
                introduced: Some(introduced.into()),
                fixed: Some(fixed.into()),
                last_affected: None,
            }],
            sev: Some(Severity::Critical),
            summary: "Prototype pollution".into(),
        }
    }

    fn content(path: &str, eco: Ecosystem, text: &str) -> DepFileContent {
        DepFileContent {
            path: path.into(),
            ecosystem: eco,
            kind: DepFileKind::Lock,
            content: Some(text.into()),
        }
    }

    /// 带漏洞库的配置（构造时即赋值，避免 default 后再改字段）。
    fn config_with(entries: Vec<Advisory>) -> ScanConfig {
        ScanConfig {
            vuln_index: Some(Arc::new(VulnIndex::new(entries))),
            ..Default::default()
        }
    }

    /// 命中：Cargo.lock 中的 serde 1.0.50 落在公告区间内 → Critical。
    #[test]
    fn 漏洞_命中版本产出Critical结论() {
        let snap = zero_snapshot();
        let config = config_with(vec![adv(
            "RUSTSEC-2024-0001",
            VulnEcosystem::Cargo,
            "serde",
            "1.0.0",
            "1.0.100",
        )]);
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![dep(
                "Cargo.lock",
                Ecosystem::Rust,
                DepFileKind::Lock,
                true,
                true,
            )],
            vec![content(
                "Cargo.lock",
                Ecosystem::Rust,
                "[[package]]\nname = \"serde\"\nversion = \"1.0.50\"\n",
            )],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].id, "deps-health.vuln:RUSTSEC-2024-0001:serde");
        assert_eq!(findings[0].severity, Severity::Critical);
        assert_eq!(findings[0].category, Category::Deps);
        assert!(
            findings[0].title.contains("serde 1.0.50"),
            "{}",
            findings[0].title
        );
        assert!(findings[0]
            .evidence
            .contains("受影响区间: [1.0.0, 1.0.100)"));
        assert!(
            findings[0].suggestion.contains("升级 serde"),
            "建议应可执行"
        );
    }

    /// 不命中：已修复版本（1.0.100）不产出结论。
    #[test]
    fn 漏洞_已修复版本不产出结论() {
        let snap = zero_snapshot();
        let config = config_with(vec![adv(
            "RUSTSEC-2024-0001",
            VulnEcosystem::Cargo,
            "serde",
            "1.0.0",
            "1.0.100",
        )]);
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![dep(
                "Cargo.lock",
                Ecosystem::Rust,
                DepFileKind::Lock,
                true,
                true,
            )],
            vec![content(
                "Cargo.lock",
                Ecosystem::Rust,
                "[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\n",
            )],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// 生态隔离：npm 公告不得命中 Rust 同名包。
    #[test]
    fn 漏洞_生态不隔离会误报() {
        let snap = zero_snapshot();
        let config = config_with(vec![adv(
            "GHSA-npm-1",
            VulnEcosystem::Npm,
            "serde",
            "0",
            "99",
        )]);
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![dep(
                "Cargo.lock",
                Ecosystem::Rust,
                DepFileKind::Lock,
                true,
                true,
            )],
            vec![content(
                "Cargo.lock",
                Ecosystem::Rust,
                "[[package]]\nname = \"serde\"\nversion = \"1.0.50\"\n",
            )],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty(), "跨生态不得命中");
    }

    /// 无漏洞库：即使有内容切片也不产出任何漏洞结论（可选增强不降级为必报）。
    #[test]
    fn 漏洞_无漏洞库时不产出结论() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![dep(
                "Cargo.lock",
                Ecosystem::Rust,
                DepFileKind::Lock,
                true,
                true,
            )],
            vec![content(
                "Cargo.lock",
                Ecosystem::Rust,
                "[[package]]\nname = \"serde\"\nversion = \"1.0.50\"\n",
            )],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// plan 契约：未接入漏洞库时不请求内容切片（省掉一次依赖文件读取）。
    #[test]
    fn 漏洞_plan按需请求内容切片() {
        let snap = zero_snapshot();
        let plain = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &plain);
        let slices = DepsHealthChecker.plan(&ctx).slices().to_vec();
        assert_eq!(slices, vec![DataSlice::DependencyFiles]);

        let with_db = config_with(vec![]);
        let ctx = ScanContext::base(&snap, &with_db);
        let slices = DepsHealthChecker.plan(&ctx).slices().to_vec();
        assert_eq!(
            slices,
            vec![
                DataSlice::DependencyFiles,
                DataSlice::DependencyFileContents
            ]
        );
    }

    /// 内容缺失（超预算 / 二进制）时不产出结论，且不 panic。
    #[test]
    fn 漏洞_内容缺失不panic() {
        let snap = zero_snapshot();
        let config = config_with(vec![adv(
            "GHSA-1",
            VulnEcosystem::Cargo,
            "serde",
            "0",
            "99",
        )]);
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![dep(
                "Cargo.lock",
                Ecosystem::Rust,
                DepFileKind::Lock,
                true,
                true,
            )],
            vec![DepFileContent {
                path: "Cargo.lock".into(),
                ecosystem: Ecosystem::Rust,
                kind: DepFileKind::Lock,
                content: None,
            }],
        );
        assert!(DepsHealthChecker.check(&ctx).is_empty());
    }

    /// 同一公告 + 同一包在清单与锁定文件中版本写法不同时只产出一个结论，
    /// 且采用锁定文件的实际安装版本（去重口径不含版本）。
    #[test]
    fn 漏洞_清单与锁定版本写法不同时只产出一个结论() {
        let snap = zero_snapshot();
        let config = config_with(vec![adv(
            "GHSA-1",
            VulnEcosystem::Npm,
            "lodash",
            "0",
            "4.17.21",
        )]);
        let ctx = vuln_ctx(
            &snap,
            &config,
            vec![
                dep(
                    "package.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Manifest,
                    true,
                    true,
                ),
                dep(
                    "package-lock.json",
                    Ecosystem::JavaScript,
                    DepFileKind::Lock,
                    true,
                    true,
                ),
            ],
            vec![
                DepFileContent {
                    path: "package-lock.json".into(),
                    ecosystem: Ecosystem::JavaScript,
                    kind: DepFileKind::Lock,
                    content: Some(r#"{"dependencies":{"lodash":{"version":"4.17.20"}}}"#.into()),
                },
                DepFileContent {
                    path: "package.json".into(),
                    ecosystem: Ecosystem::JavaScript,
                    kind: DepFileKind::Manifest,
                    content: Some(r#"{"dependencies":{"lodash":"^4.17.20"}}"#.into()),
                },
            ],
        );
        let findings = DepsHealthChecker.check(&ctx);
        assert_eq!(findings.len(), 1, "去重后应只剩一条: {findings:?}");
        assert_eq!(findings[0].id, "deps-health.vuln:GHSA-1:lodash");
        assert!(
            findings[0].title.contains("4.17.20"),
            "{}",
            findings[0].title
        );
    }

    /// 端到端：接入漏洞库后，依赖维度的漏洞结论参与评分（Critical −15）。
    #[test]
    fn 端到端_漏洞结论参与评分() {
        let mut t = TestRepo::init();
        t.commit(
            &[
                ("Cargo.toml", b"[package]\nname = \"x\"\n"),
                (
                    "Cargo.lock",
                    b"[[package]]\nname = \"serde\"\nversion = \"1.0.50\"\n",
                ),
                (
                    "package.json",
                    b"{\"dependencies\":{\"lodash\":\"^4.17.20\"}}",
                ),
                (
                    "package-lock.json",
                    b"{\"dependencies\":{\"lodash\":{\"version\":\"4.17.20\"}}}",
                ),
            ],
            "init",
        );
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(DepsHealthChecker)).unwrap();

        let config = config_with(vec![
            adv(
                "RUSTSEC-1",
                VulnEcosystem::Cargo,
                "serde",
                "1.0.0",
                "1.0.100",
            ),
            adv("GHSA-1", VulnEcosystem::Npm, "lodash", "0", "4.17.21"),
        ]);
        let report =
            crate::engine::scan::scan_with_progress(t.path(), &registry, None, None, &config)
                .unwrap();
        let vulns: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.id.starts_with("deps-health.vuln:"))
            .collect();
        assert_eq!(vulns.len(), 2, "两个生态各命中一条: {vulns:?}");
        assert_eq!(report.scores.deps, Some(100 - 15 * 2), "两条 Critical");
    }
}
