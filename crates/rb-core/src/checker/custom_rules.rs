//! 自定义规则检查器：按用户配置的正则 + 路径 glob 扫描工作区文件文本。
//!
//! 定位：检查器插件机制的轻量替代（`docs/roadmap.md` 中期任务）。
//!
//! 约定：
//! - 规则需**先经 [`crate::rules::compile_rules`] 校验编译**，本检查器不重复校验；
//! - 无规则时不声明任何切片（空跑不产生采集开销），且注册方应直接不注册本检查器
//!   ——否则会把「无任何规则的维度」错误地标记为已覆盖；
//! - 命中按「规则 × 文件」聚合为一条结论，evidence 中的匹配样本一律经
//!   [`mask_secret`] 掩码（一处掩码，处处掩码）；
//! - 单条规则最多产出 [`MAX_FINDINGS_PER_RULE`] 条文件级结论，超出时追加一条
//!   如实的「已截断」提示，避免宽泛正则淹没报告。

use crate::checker::plan::{DataSlice, ScanPlan};
use crate::checker::secrets::mask_secret;
use crate::checker::{Checker, ScanContext, SliceData};
use crate::model::finding::{Category, Finding, Severity};
use crate::rules::{compile_rules, CompiledRule, CustomRule};
use crate::Result;

/// 工作区文件内容切片的模式：与其它文本检查器共用 `all`，合并去重后只采集一次。
const ALL_PATTERN: &str = "all";

/// 单条规则最多产出的文件级结论数。
const MAX_FINDINGS_PER_RULE: usize = 50;

/// evidence 中列出的行号上限（超出折叠为省略号）。
const MAX_LINES: usize = 5;

/// 自定义规则检查器（持有已编译规则，纯匹配、无 I/O）。
pub struct CustomRulesChecker {
    /// 已编译规则集（顺序与用户配置一致）。
    rules: Vec<CompiledRule>,
}

impl CustomRulesChecker {
    /// 由规则原文构造：批量校验并编译（含 id 唯一性）。
    pub fn new(rules: Vec<CustomRule>) -> Result<Self> {
        Ok(Self {
            rules: compile_rules(&rules)?,
        })
    }

    /// 由已编译规则构造（壳层已校验过时复用，避免重复编译）。
    pub fn from_compiled(rules: Vec<CompiledRule>) -> Self {
        Self { rules }
    }

    /// 规则条数。
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Checker for CustomRulesChecker {
    fn id(&self) -> &'static str {
        "custom-rules"
    }

    /// 单值维度口径：取首条规则的维度（仅为 trait 兼容）。
    ///
    /// 自定义规则可逐条指定维度，**覆盖度以 [`Checker::categories`] 为准**。
    fn category(&self) -> Category {
        self.rules
            .first()
            .map(|r| r.rule().category)
            .unwrap_or(Category::Structure)
    }

    /// 本检查器覆盖的全部维度（规则维度去重，按首次出现序）。
    fn categories(&self) -> Vec<Category> {
        let mut out: Vec<Category> = Vec::new();
        for r in &self.rules {
            if !out.contains(&r.rule().category) {
                out.push(r.rule().category);
            }
        }
        out
    }

    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        // 无规则不声明切片：扫描计划为空，采集阶段零开销。
        if !self.rules.is_empty() {
            plan.push(DataSlice::FileContents {
                pattern: ALL_PATTERN.to_owned(),
            });
        }
        plan
    }

    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let slice = DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        };
        let Some(SliceData::FileContents(files)) = ctx.slice(&slice) else {
            return Vec::new();
        };

        let mut findings = Vec::new();
        for compiled in &self.rules {
            let mut matched_files = 0usize;
            for file in files {
                if !compiled.path_allowed(&file.path) {
                    continue;
                }
                // 二进制 / 超预算 / 读取失败的文件无内容，天然不参与匹配。
                let Some(content) = file.content.as_deref() else {
                    continue;
                };
                // 命中总数为真实值；偏移只保留前若干处（见 find_offsets 的上限说明）。
                let (offsets, hits) = compiled.find_offsets(content);
                if hits == 0 {
                    continue;
                }
                matched_files += 1;
                if matched_files <= MAX_FINDINGS_PER_RULE {
                    findings.push(file_finding(compiled, &file.path, content, &offsets, hits));
                }
            }
            if matched_files > MAX_FINDINGS_PER_RULE {
                findings.push(truncated_finding(compiled, matched_files));
            }
        }
        findings
    }
}

/// 单文件命中结论：id / 严重度 / 维度 / 标题 / 建议取自规则，证据为该文件的命中摘要。
///
/// `hits` 为该文件的真实命中总数（`offsets` 可能只保留前若干处），证据里的
/// 「命中 N 处」必须如实反映总数，不能因为行号被截断而缩水。
fn file_finding(
    compiled: &CompiledRule,
    path: &str,
    content: &str,
    offsets: &[usize],
    hits: usize,
) -> Finding {
    let rule = compiled.rule();
    // 同一行多处命中只列一次行号（否则会出现「行 1, 1, 1」这类噪音）。
    let mut lines: Vec<usize> = offsets
        .iter()
        .take(MAX_LINES)
        .map(|&off| line_number_at(content, off))
        .collect();
    lines.sort_unstable();
    lines.dedup();
    let more = if hits > MAX_LINES { "…" } else { "" };
    let line_text = lines
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    // 样本取自首个匹配并立即掩码：用户正则可能命中密钥，证据不得回传原文。
    let sample = compiled
        .regex()
        .find(content)
        .map(|m| mask_secret(m.as_str()))
        .unwrap_or_else(|| "（未取到样本）".to_owned());

    Finding {
        id: format!("custom-rules.{}:{path}", rule.id),
        severity: rule.severity,
        category: rule.category,
        title: rule.title.clone(),
        evidence: format!("文件: {path}\n命中: {hits} 处（行 {line_text}{more}）\n样例: {sample}"),
        suggestion: rule.suggestion.clone(),
    }
}

/// 命中文件数超出上限时的如实提示（避免用户误以为全部命中都已列出）。
fn truncated_finding(compiled: &CompiledRule, matched_files: usize) -> Finding {
    let rule = compiled.rule();
    Finding {
        id: format!("custom-rules.{}:__truncated__", rule.id),
        severity: Severity::Info,
        category: rule.category,
        title: format!("「{}」命中文件过多", rule.title),
        evidence: format!(
            "规则 {} 共命中 {matched_files} 个文件，仅列出前 {MAX_FINDINGS_PER_RULE} 个",
            rule.id
        ),
        suggestion: "收窄路径限定或改用更精确的正则，避免结论淹没报告".to_owned(),
    }
}

/// 字节偏移 → 1 起的行号（按 `\n` 计数；偏移来自正则匹配，必为字符边界）。
fn line_number_at(content: &str, byte_offset: usize) -> usize {
    content
        .as_bytes()
        .iter()
        .take(byte_offset)
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::checker::ScanConfig;
    use crate::model::finding::Severity;
    use crate::model::slice_meta::FileContent;
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

    /// 构造带 FileContents 切片的上下文。
    fn ctx_with_content<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        files: Vec<(&str, Option<&str>)>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        let contents: Vec<FileContent> = files
            .into_iter()
            .map(|(path, content)| FileContent {
                path: path.into(),
                size: content.map(|c| c.len() as u64).unwrap_or(0),
                content: content.map(str::to_owned),
            })
            .collect();
        ctx.insert_slice(
            DataSlice::FileContents {
                pattern: ALL_PATTERN.into(),
            },
            SliceData::FileContents(contents),
        );
        ctx
    }

    fn rule(id: &str, pattern: &str) -> CustomRule {
        CustomRule {
            id: id.into(),
            title: format!("命中 {id}"),
            severity: Severity::Warning,
            category: Category::Structure,
            pattern: pattern.into(),
            path_glob: None,
            ignore_case: false,
            suggestion: "按规则建议整改".into(),
        }
    }

    /// 命中：单文件多处命中聚合为一条结论，evidence 含行号与掩码样本。
    #[test]
    fn 命中_单文件多处聚合为一条结论() {
        let checker = CustomRulesChecker::new(vec![rule("no-console", r"console\.log")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(
            &snap,
            &config,
            vec![("src/a.ts", Some("console.log(1)\nconsole.log(2)\n"))],
        );
        let findings = checker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "custom-rules.no-console:src/a.ts");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].category, Category::Structure);
        assert!(
            findings[0].evidence.contains("命中: 2 处"),
            "{}",
            findings[0].evidence
        );
        assert!(
            findings[0].evidence.contains("行 1, 2"),
            "{}",
            findings[0].evidence
        );
    }

    /// 不命中：无匹配文件时不产出任何结论。
    #[test]
    fn 不命中_无匹配不产出结论() {
        let checker = CustomRulesChecker::new(vec![rule("no-console", r"console\.log")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(
            &snap,
            &config,
            vec![("src/a.ts", Some("println!(\"hi\")\n"))],
        );
        assert!(checker.check(&ctx).is_empty());
    }

    /// 二进制 / 无内容文件跳过：content 为 None 的文件不参与匹配。
    #[test]
    fn 无内容文件_跳过不误报() {
        let checker = CustomRulesChecker::new(vec![rule("no-console", r"console\.log")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(&snap, &config, vec![("bin.dat", None)]);
        assert!(checker.check(&ctx).is_empty());
    }

    /// 路径 glob 限定：目录外文件命中规则也不产出结论。
    #[test]
    fn 路径限定_目录外文件不产出结论() {
        let mut r = rule("no-console", r"console\.log");
        r.path_glob = Some("src/*.ts".into());
        let checker = CustomRulesChecker::new(vec![r]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(
            &snap,
            &config,
            vec![
                ("src/a.ts", Some("console.log(1)\n")),
                ("tests/a.ts", Some("console.log(1)\n")),
            ],
        );
        let findings = checker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.ends_with("src/a.ts"), "{}", findings[0].id);
    }

    /// 忽略规则：切片已在采集层按 `.repobalance-ignore` 过滤，被忽略文件不产出结论。
    #[test]
    fn 忽略规则_被忽略文件不产出结论() {
        let mut t = TestRepo::init();
        t.commit(
            &[
                ("src/a.ts", b"console.log(1)\n"),
                ("dist/b.ts", b"console.log(2)\n"),
            ],
            "init",
        );
        std::fs::write(t.path().join(".repobalance-ignore"), "dist/\n").unwrap();
        let mut registry = crate::checker::CheckerRegistry::new();
        registry
            .register(Box::new(
                CustomRulesChecker::new(vec![rule("no-console", r"console\.log")]).unwrap(),
            ))
            .unwrap();
        let report = crate::engine::scan::scan(t.path(), &registry).unwrap();
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        assert!(report.findings[0].id.ends_with("src/a.ts"));
    }

    /// 边界：命中文件数超上限时截断，并追加一条如实的提示结论。
    #[test]
    fn 边界_超出上限时截断并提示() {
        let checker = CustomRulesChecker::new(vec![rule("broad", "hit")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let files: Vec<(String, String)> = (0..(MAX_FINDINGS_PER_RULE + 3))
            .map(|i| (format!("src/f{i}.ts"), "hit\n".to_owned()))
            .collect();
        let refs: Vec<(&str, Option<&str>)> = files
            .iter()
            .map(|(p, c)| (p.as_str(), Some(c.as_str())))
            .collect();
        let ctx = ctx_with_content(&snap, &config, refs);
        let findings = checker.check(&ctx);
        assert_eq!(findings.len(), MAX_FINDINGS_PER_RULE + 1);
        let last = findings.last().unwrap();
        assert_eq!(last.id, "custom-rules.broad:__truncated__");
        assert_eq!(last.severity, Severity::Info);
        assert!(
            last.evidence
                .contains(&format!("共命中 {}", MAX_FINDINGS_PER_RULE + 3)),
            "{}",
            last.evidence
        );
    }

    /// 边界：命中总数远超行号上限时，证据里的「命中 N 处」仍是真实总数。
    #[test]
    fn 边界_海量命中时总数仍真实() {
        let checker = CustomRulesChecker::new(vec![rule("many", "x*")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        // x* 在 9 字节文本上命中 10 处（含末尾空匹配），偏移受上限约束但总数不缩水。
        let ctx = ctx_with_content(&snap, &config, vec![("a.txt", Some("abcabcabc"))]);
        let findings = checker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].evidence.contains("命中: 10 处"),
            "{}",
            findings[0].evidence
        );
    }

    /// 证据掩码：样本不回传正则命中的原文。
    #[test]
    fn 证据掩码_样本不回传原文() {
        let secret = format!("token = {}", "aB3dE6gH9jK2mN5pQ8rS1tU4vW7xY0zAq");
        let checker =
            CustomRulesChecker::new(vec![rule("token-literal", r"aB3dE6gH9jK2mN5")]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(&snap, &config, vec![("cfg.txt", Some(&secret))]);
        let findings = checker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(
            !findings[0].evidence.contains("aB3dE6gH9jK2mN5"),
            "evidence 不得包含未掩码匹配原文: {}",
            findings[0].evidence
        );
        assert!(findings[0].evidence.contains("已掩码"));
    }

    /// 无规则时计划为空（不请求任何切片，采集零开销）。
    #[test]
    fn 无规则_计划为空() {
        let checker = CustomRulesChecker::from_compiled(Vec::new());
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert!(checker.plan(&ctx).is_empty());
        assert!(checker.check(&ctx).is_empty());
        assert_eq!(checker.rule_count(), 0);
    }

    /// 有规则时声明 `all` 文件内容切片，且两类规则维度去重后全部计入覆盖度。
    #[test]
    fn 计划与覆盖度_多维度去重() {
        let mut sec = rule("no-http", r"http://");
        sec.category = Category::Security;
        let mut history_rule = rule("no-fixup", r"^fixup!");
        history_rule.category = Category::History;
        let checker =
            CustomRulesChecker::new(vec![sec, rule("no-console", "x"), history_rule]).unwrap();
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let plan = checker.plan(&ctx);
        assert_eq!(
            plan.slices(),
            &[DataSlice::FileContents {
                pattern: ALL_PATTERN.to_owned()
            }]
        );
        assert_eq!(
            checker.categories(),
            vec![Category::Security, Category::Structure, Category::History]
        );
        assert_eq!(checker.rule_count(), 3);
    }
}
