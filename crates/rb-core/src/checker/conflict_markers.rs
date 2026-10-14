//! 合并冲突残留检查器（Structure 维度）：检出工作区与历史提交中残留的
//! `<<<<<<<` / `=======` / `>>>>>>>` 冲突标记。
//!
//! 纯计算：消费 `FileContents("all")` 切片（编排层已按预算读取文本内容、
//! 过滤忽略规则），不触碰文件系统、不 import git2。
//!
//! # 判定口径（三重防误报）
//! - 完整三元组：起始标记、分隔线、结束标记按行序齐备才计为冲突块，
//!   单独 `=======`（Markdown setext 标题 / 分隔线）永不命中；
//! - 停用词白名单（与敏感信息检查同形）：标记标签命中文档示例词则跳过；
//! - 路径豁免：复用 `.repobalance-ignore`（引擎层统一过滤，本模块零改动）；
//! - 二进制与超预算文件无内容，天然不参与扫描。

use std::collections::HashSet;

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};
use crate::model::slice_meta::HistoryFileContent;

/// 合并冲突残留检查器：无状态、纯计算。
pub struct ConflictMarkersChecker;

/// 文件内容切片的固定模式（与 plan 声明一致）。
const ALL_PATTERN: &str = "all";

/// 冲突标记定长（git 约定恰好 7 个字符）。
const MARKER_LEN: usize = 7;

/// 单文件冲突块展示上限。
const BLOCK_PREVIEW: usize = 8;

/// 工作区口径的可执行修复建议。
const WORKTREE_SUGGESTION: &str = "在编辑器中搜索 <<<<<<< 定位冲突块并完成解决后提交；可用 git diff --check 复核是否仍有残留标记，若合并未完成可先 git merge --abort 回退";

/// 历史口径的说明（含可选的历史清除命令；冲突标记不值得为此重写历史）。
fn history_suggestion(path: &str) -> String {
    format!(
        "该冲突标记仅存在于历史提交中（工作区已解决），通常无需处理；如确需从历史清除，可用 git filter-repo --path {path} --invert-paths（执行前请备份仓库）"
    )
}

/// 一个完整冲突块：起始标记与结束标记的行号、标签。
struct ConflictBlock {
    /// 起始标记所在行（1 起）。
    start_line: usize,
    /// 起始标记标签（`<<<<<<<` 之后的文本，可为空）。
    start_label: String,
    /// 结束标记所在行（1 起）。
    end_line: usize,
    /// 结束标记标签（`>>>>>>>` 之后的文本，可为空）。
    end_label: String,
}

impl ConflictBlock {
    /// 起始标记样本（含标签与行号）。
    fn sample_start(&self) -> String {
        sample_marker("<<<<<<<", &self.start_label, self.start_line)
    }

    /// 结束标记样本（含标签与行号）。
    fn sample_end(&self) -> String {
        sample_marker(">>>>>>>", &self.end_label, self.end_line)
    }
}

impl Checker for ConflictMarkersChecker {
    fn id(&self) -> &'static str {
        "conflict-markers"
    }

    fn category(&self) -> Category {
        Category::Structure
    }

    /// 幂等声明两切片：工作区文件内容 + 历史文件内容（去重后携带文本内容）。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        });
        plan.push(DataSlice::HistoryFileContents);
        plan
    }

    /// 双口径扫描：工作区命中（Warning）+ 历史残留（Info）。去重口径：
    /// 仅当同一路径在工作区仍有冲突块时抑制历史结论（避免同一现场双重报告）；
    /// 工作区已修复的历史残留照常报告。切片未就绪时防御性返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let all_slice = DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        };
        let mut findings = Vec::new();
        let mut conflicted_paths: HashSet<String> = HashSet::new();

        if let Some(SliceData::FileContents(files)) = ctx.slice(&all_slice) {
            for file in files {
                let Some(content) = file.content.as_deref() else {
                    continue; // 二进制 / 超预算：无内容可扫
                };
                if !find_blocks(content).is_empty() {
                    conflicted_paths.insert(file.path.clone());
                }
                findings.extend(scan_file(&file.path, content));
            }
        }

        let history_slice = DataSlice::HistoryFileContents;
        if let Some(SliceData::HistoryFileContents(files)) = ctx.slice(&history_slice) {
            for file in files {
                if conflicted_paths.contains(&file.path) {
                    continue;
                }
                if let Some(finding) = history_finding(file) {
                    findings.push(finding);
                }
            }
        }

        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 扫描单个文件内容，产出工作区 finding（无完整冲突块时返回空列表）。
fn scan_file(path: &str, content: &str) -> Vec<Finding> {
    let blocks = find_blocks(content);
    if blocks.is_empty() {
        return Vec::new();
    }
    vec![worktree_finding(path, &blocks)]
}

/// 按行序提取完整冲突三元组；未闭合 / 缺分隔线 / 停用词标签的片段一律丢弃。
fn find_blocks(content: &str) -> Vec<ConflictBlock> {
    let mut blocks = Vec::new();
    // 当前未闭合块：(起始行, 起始标签, 已见分隔线)。
    let mut current: Option<(usize, String, bool)> = None;
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        if is_start_marker(line) {
            // 未闭合块中的新起始标记：取最新一处为当前块（外层视为丢弃）。
            current = Some((line_no, marker_label(line).to_owned(), false));
        } else if is_sep_marker(line) {
            if let Some(entry) = current.as_mut() {
                entry.2 = true;
            }
        } else if is_end_marker(line) {
            let Some((start_line, start_label, sep_seen)) = current.take() else {
                continue;
            };
            if !sep_seen {
                continue;
            }
            let end_label = marker_label(line);
            if is_stopword(&start_label) || is_stopword(end_label) {
                continue;
            }
            blocks.push(ConflictBlock {
                start_line,
                start_label,
                end_line: line_no,
                end_label: end_label.to_owned(),
            });
        }
    }
    blocks
}

/// 组装工作区冲突残留 finding（Warning）。
fn worktree_finding(path: &str, blocks: &[ConflictBlock]) -> Finding {
    let lines: Vec<String> = blocks
        .iter()
        .take(BLOCK_PREVIEW)
        .map(|b| b.start_line.to_string())
        .collect();
    let more = if blocks.len() > BLOCK_PREVIEW {
        format!(" 等 {}", blocks.len())
    } else {
        String::new()
    };
    let first = &blocks[0];
    Finding {
        id: format!("conflict-markers.worktree:{path}"),
        severity: Severity::Warning,
        category: Category::Structure,
        title: format!("工作区存在未解决的合并冲突标记: {path}"),
        evidence: format!(
            "文件: {path}\n冲突块: {n} 处\n起始行: {}{more}\n首块样本: {start} → {end}",
            lines.join(", "),
            n = blocks.len(),
            start = first.sample_start(),
            end = first.sample_end(),
        ),
        suggestion: WORKTREE_SUGGESTION.to_owned(),
    }
}

/// 组装历史残留 finding（Info）；无完整冲突块时返回 None。
fn history_finding(file: &HistoryFileContent) -> Option<Finding> {
    let content = file.content.as_deref()?;
    let blocks = find_blocks(content);
    if blocks.is_empty() {
        return None;
    }
    let lines: Vec<String> = blocks
        .iter()
        .take(BLOCK_PREVIEW)
        .map(|b| b.start_line.to_string())
        .collect();
    let more = if blocks.len() > BLOCK_PREVIEW {
        format!(" 等 {}", blocks.len())
    } else {
        String::new()
    };
    let first = &blocks[0];
    Some(Finding {
        id: format!("conflict-markers.history:{}", file.path),
        severity: Severity::Info,
        category: Category::Structure,
        title: format!("历史提交中残留冲突标记（已不在工作区）: {}", file.path),
        evidence: format!(
            "文件: {path}\n冲突块: {n} 处\n起始行: {}{more}\n首块样本: {start} → {end}\n首次引入: {hash} {summary}",
            lines.join(", "),
            n = blocks.len(),
            start = first.sample_start(),
            end = first.sample_end(),
            path = file.path,
            hash = short_hash(&file.first_commit.id),
            summary = file.first_commit.summary,
        ),
        suggestion: history_suggestion(&file.path),
    })
}

/// 提交 hash 前 8 个字符（不足则取全部；按字符切片避免 UTF-8 panic）。
fn short_hash(id: &str) -> &str {
    id.get(..id.char_indices().nth(8).map(|(i, _)| i).unwrap_or(id.len()))
        .unwrap_or(id)
}

/// 行是否为冲突起始标记（`<<<<<<<`，行尾为空或空格 + 标签）。
fn is_start_marker(line: &str) -> bool {
    marker_line(line, b'<')
}

/// 行是否为冲突分隔线（git 约定恰好 7 个 `=`；8 个 `=` 的装饰线不判为分隔）。
fn is_sep_marker(line: &str) -> bool {
    line == "======="
}

/// 行是否为冲突结束标记（`>>>>>>>`，行尾为空或空格 + 标签）。
fn is_end_marker(line: &str) -> bool {
    marker_line(line, b'>')
}

/// 标记行通用判定：前 7 字符为重复的 `{ch}`，其后为空或空格 + 标签
/// （git 产出约定；8 个 `<` 的装饰线不判为标记）。
fn marker_line(line: &str, ch: u8) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= MARKER_LEN
        && bytes[..MARKER_LEN].iter().all(|&b| b == ch)
        && (bytes.len() == MARKER_LEN || bytes[MARKER_LEN] == b' ')
}

/// 提取标记标签（标记前缀之后的文本）；无标签返回空串。
///
/// 标签前必有单个空格（[`marker_line`] 已保证），按字节切片落在字符边界上。
fn marker_label(line: &str) -> &str {
    let bytes = line.as_bytes();
    if bytes.len() > MARKER_LEN {
        line[MARKER_LEN + 1..].trim()
    } else {
        ""
    }
}

/// 停用词白名单：标签命中则视为文档示例/占位标记，不产出结论（形态对齐
/// 敏感信息检查的 `is_stopword`：常量表 + 大小写不敏感包含匹配）。
fn is_stopword(label: &str) -> bool {
    if label.is_empty() {
        return false;
    }
    let lower = label.to_ascii_lowercase();
    const STOPWORDS: [&str; 7] = [
        "example",
        "sample",
        "placeholder",
        "not-a-real",
        "changeme",
        "dummy",
        "tutorial",
    ];
    STOPWORDS.iter().any(|s| lower.contains(s))
}

/// 标记样本渲染：`<<<<<<< feat-x（行 12）`；无标签渲染为 `<<<<<<<（行 12）`。
fn sample_marker(marker: &str, label: &str, line: usize) -> String {
    if label.is_empty() {
        format!("{marker}（行 {line}）")
    } else {
        format!("{marker} {label}（行 {line}）")
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{
        find_blocks, is_stopword, marker_label, scan_file, ConflictMarkersChecker, ALL_PATTERN,
    };
    use crate::checker::{Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData};
    use crate::engine::scan::scan;
    use crate::model::finding::Severity;
    use crate::model::slice_meta::HistoryFileContent;
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

    /// 典型冲突块内容（HEAD 与 feature 两侧）。
    fn conflict_content() -> String {
        "<<<<<<< HEAD\n本地改动\n=======\n对方改动\n>>>>>>> feat-x\n".into()
    }

    fn ctx_with_content<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        files: Vec<(&str, Option<&str>)>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        let contents: Vec<crate::model::slice_meta::FileContent> = files
            .into_iter()
            .map(|(path, content)| crate::model::slice_meta::FileContent {
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

    /// 构造历史内容条目（提交时间固定、仅 id/摘要参与断言）。
    fn hist_file(
        path: &str,
        content: Option<&str>,
        cid: &str,
        summary: &str,
    ) -> HistoryFileContent {
        HistoryFileContent {
            path: path.into(),
            size: content.map(|c| c.len() as u64).unwrap_or(0),
            first_commit: crate::model::snapshot::CommitSummary {
                id: cid.into(),
                summary: summary.into(),
                author: "fixture".into(),
                time: 1_700_000_000,
                time_offset: 0,
            },
            content: content.map(str::to_owned),
        }
    }

    /// 同时注入工作区与历史内容切片。
    fn ctx_with_slices<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        workdir: Vec<(&str, Option<&str>)>,
        history: Vec<HistoryFileContent>,
    ) -> ScanContext<'a> {
        let mut ctx = ctx_with_content(snapshot, config, workdir);
        ctx.insert_slice(
            DataSlice::HistoryFileContents,
            SliceData::HistoryFileContents(history),
        );
        ctx
    }

    fn ids(findings: &[crate::model::finding::Finding]) -> Vec<String> {
        findings.iter().map(|f| f.id.clone()).collect()
    }

    // ===== 命中 =====

    /// 完整冲突三元组 → Warning，id / 证据 / 建议齐全。
    #[test]
    fn 命中_完整冲突三元组_产出Warning() {
        let findings = scan_file("src/a.rs", &conflict_content());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].id.starts_with("conflict-markers.worktree:"));
        assert!(findings[0].evidence.contains("冲突块: 1 处"));
        assert!(findings[0].evidence.contains("<<<<<<< HEAD（行 1）"));
        assert!(findings[0].evidence.contains(">>>>>>> feat-x（行 5）"));
        assert!(findings[0].suggestion.contains("git diff --check"));
    }

    /// 同文件多个冲突块 → 计数与多起始行预览齐全。
    #[test]
    fn 命中_多冲突块_计数与起始行预览齐全() {
        let content = format!(
            "{}\n普通行\n{}",
            conflict_content(),
            "<<<<<<< main\na\n=======\nb\n>>>>>>> other\n"
        );
        let findings = scan_file("src/b.rs", &content);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].evidence.contains("冲突块: 2 处"));
        assert!(findings[0].evidence.contains("起始行: 1, 8"));
    }

    /// 无标签裸标记（`<<<<<<<` 与 `>>>>>>>` 后无文本）仍判为冲突块。
    #[test]
    fn 命中_无标签裸标记_仍产出结论() {
        let findings = scan_file("c.txt", "<<<<<<<\nx\n=======\ny\n>>>>>>>\n");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].evidence.contains("<<<<<<<（行 1）"));
    }

    // ===== 不命中 =====

    /// 单独 `=======`（Markdown setext 标题 / 分隔线）不产出结论。
    #[test]
    fn 不命中_单独分隔线_返回空() {
        assert!(scan_file("README.md", "# 标题\n=======\n正文\n").is_empty());
        assert!(scan_file("a.md", "章节\n=====\n内容\n").is_empty());
    }

    /// 8 个字符的装饰线（`<<<<<<<<` / `>>>>>>>>`）不判为标记。
    #[test]
    fn 不命中_八字符装饰线_返回空() {
        let content = "<<<<<<<<<\nx\n=======\ny\n>>>>>>>>\n";
        assert!(scan_file("a.txt", content).is_empty());
    }

    /// 未闭合三元组（缺结束标记 / 缺分隔线）不产出结论。
    #[test]
    fn 不命中_未闭合三元组_返回空() {
        assert!(scan_file("a.txt", "<<<<<<< HEAD\n只有起始\n").is_empty());
        assert!(scan_file("b.txt", "<<<<<<< HEAD\n缺分隔\n>>>>>>> feat\n").is_empty());
        assert!(scan_file("c.txt", "=======\n只有分隔\n>>>>>>> feat\n").is_empty());
    }

    /// 停用词标签（文档示例值）不产出结论。
    #[test]
    fn 不命中_停用词标签_返回空() {
        let content = "<<<<<<< example\na\n=======\nb\n>>>>>>> sample\n";
        assert!(scan_file("docs/guide.md", content).is_empty());
        let content2 = "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> placeholder\n";
        assert!(scan_file("docs/demo.md", content2).is_empty());
    }

    /// 普通文本（无标记形态）返回空。
    #[test]
    fn 不命中_普通文本_返回空() {
        let findings = scan_file(
            "README.md",
            "# 项目\n这是一个示例仓库，用于演示体检功能。\n======= 分隔线出现在行中\n",
        );
        assert!(findings.is_empty());
    }

    // ===== 边界 =====

    /// 边界：标记必须位于行首（前导空格不算），标签按空格切分。
    #[test]
    fn 边界_标记行首判定与标签提取() {
        assert!(scan_file("a.txt", " <<<<<<< HEAD\nx\n=======\ny\n>>>>>>> feat\n").is_empty());
        assert_eq!(marker_label("<<<<<<< feat-x"), "feat-x");
        assert_eq!(marker_label("<<<<<<<"), "");
        assert_eq!(marker_label(">>>>>>>   spaced  "), "spaced");
    }

    /// 边界：未闭合块中的新起始标记取最新一处。
    #[test]
    fn 边界_嵌套起始标记_取最新一处() {
        let content = "<<<<<<< old\nx\n<<<<<<< new\ny\n=======\nz\n>>>>>>> feat\n";
        let blocks = find_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_line, 3);
        assert_eq!(blocks[0].start_label, "new");
    }

    /// 边界：停用词函数对空标签与大小写稳定。
    #[test]
    fn 边界_停用词口径() {
        assert!(!is_stopword(""), "空标签（裸标记）不停用");
        assert!(is_stopword("EXAMPLE"));
        assert!(is_stopword("my-sample-branch"));
        assert!(!is_stopword("feat-x"));
    }

    /// 二进制/超预算文件（content 为 None）→ 跳过不扫。
    #[test]
    fn 不命中_无内容文件_跳过() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(
            &snap,
            &config,
            vec![("blob.bin", None), ("a.rs", Some(&conflict_content()))],
        );
        let findings = ConflictMarkersChecker.check(&ctx);
        assert_eq!(findings.len(), 1, "仅对有内容文件产出结论");
        assert!(findings[0].id.contains("a.rs"));
    }

    // ===== 历史口径 =====

    /// 历史残留（工作区已无该路径）→ Info，含首次引入提交与 filter-repo 可选建议。
    #[test]
    fn 命中_历史残留_产出Info含首次引入() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_slices(
            &snap,
            &config,
            vec![],
            vec![hist_file(
                "old.rs",
                Some(&conflict_content()),
                "abcdef1234567890",
                "merge feat-x",
            )],
        );
        let findings = ConflictMarkersChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
        assert!(findings[0].id.starts_with("conflict-markers.history:"));
        assert!(findings[0].title.contains("已不在工作区"));
        assert!(findings[0]
            .evidence
            .contains("首次引入: abcdef12 merge feat-x"));
        assert!(findings[0].suggestion.contains("git filter-repo"));
        assert!(findings[0].suggestion.contains("通常无需处理"));
    }

    /// 同一路径工作区已报告冲突块 → 历史口径不重复报告。
    #[test]
    fn 不命中_工作区已报同路径_历史不重复报告() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_slices(
            &snap,
            &config,
            vec![("a.rs", Some(&conflict_content()))],
            vec![hist_file(
                "a.rs",
                Some(&conflict_content()),
                "abcdef12",
                "merge",
            )],
        );
        let findings = ConflictMarkersChecker.check(&ctx);
        assert_eq!(findings.len(), 1, "仅保留工作区口径");
        assert!(findings[0].id.starts_with("conflict-markers.worktree:"));
    }

    /// 同一路径工作区已修复（无冲突块）→ 历史残留照常报告。
    #[test]
    fn 命中_同路径工作区已修复_历史照常报告() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_slices(
            &snap,
            &config,
            vec![("a.rs", Some("resolved\n"))],
            vec![hist_file(
                "a.rs",
                Some(&conflict_content()),
                "abcdef12",
                "merge",
            )],
        );
        let findings = ConflictMarkersChecker.check(&ctx);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("conflict-markers.history:"));
    }

    /// 历史条目无内容（二进制 / 超预算）→ 跳过。
    #[test]
    fn 不命中_历史条目无内容_跳过() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_slices(
            &snap,
            &config,
            vec![],
            vec![hist_file("old.bin", None, "abcdef12", "merge")],
        );
        assert!(ConflictMarkersChecker.check(&ctx).is_empty());
    }

    /// 历史内容无冲突块（干净 blob）→ 不产出。
    #[test]
    fn 不命中_历史内容干净_返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_slices(
            &snap,
            &config,
            vec![],
            vec![hist_file("old.rs", Some("resolved\n"), "abcdef12", "merge")],
        );
        assert!(ConflictMarkersChecker.check(&ctx).is_empty());
    }

    // ===== plan / check 契约 =====

    /// plan 声明幂等：两次调用等价，切片恰为 FileContents("all") + HistoryFileContents。
    #[test]
    fn plan声明幂等_含FileContents与HistoryFileContents切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = ConflictMarkersChecker.plan(&ctx);
        let p2 = ConflictMarkersChecker.plan(&ctx);
        assert_eq!(p1, p2);
        assert_eq!(p1.slices().len(), 2);
        assert!(p1.slices().contains(&DataSlice::HistoryFileContents));
        assert!(p1
            .slices()
            .iter()
            .any(|s| matches!(s, DataSlice::FileContents { pattern } if pattern == "all")));
    }

    /// 同一实例重复 check 结果等价（纯函数）；切片缺失时防御性返回空。
    #[test]
    fn check纯函数_重复等价_切片缺失返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert!(
            ConflictMarkersChecker.check(&ctx).is_empty(),
            "切片未就绪返回空"
        );

        let ctx = ctx_with_content(&snap, &config, vec![("a.rs", Some(&conflict_content()))]);
        assert_eq!(
            ConflictMarkersChecker.check(&ctx),
            ConflictMarkersChecker.check(&ctx)
        );
    }

    // ===== 忽略规则 / 端到端 =====

    /// 忽略规则：.repobalance-ignore 忽略后不产出结论（端到端）。
    #[test]
    fn 忽略规则_被忽略的文件不产出finding() {
        let mut t = TestRepo::init();
        t.commit(
            &[("src/a.rs", conflict_content().as_bytes())],
            "add conflict",
        );
        std::fs::write(t.path().join(".repobalance-ignore"), "src/a.rs\n").unwrap();
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ConflictMarkersChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }

    /// 端到端：仓库内冲突文件 → Warning 结论；干净文件不产出。
    #[test]
    fn 端到端_冲突文件产出Warning且结构维度扣分() {
        let mut t = TestRepo::init();
        t.commit(
            &[("src/a.rs", conflict_content().as_bytes())],
            "add conflict",
        );
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ConflictMarkersChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let got = ids(&report.findings);
        assert_eq!(got, vec!["conflict-markers.worktree:src/a.rs"]);
        assert_eq!(report.findings[0].severity, Severity::Warning);
        // 仅 Structure 维度覆盖且被 Warning 扣 8 分 → structure = 92。
        assert_eq!(report.scores.structure, Some(92));
    }

    /// 端到端：先提交冲突后修复 → 仅历史口径 Info；Structure 扣 3 分。
    #[test]
    fn 端到端_历史残留产出Info且结构维度扣分() {
        let mut t = TestRepo::init();
        t.commit(
            &[("src/a.rs", conflict_content().as_bytes())],
            "add conflict",
        );
        t.commit(&[("src/a.rs", b"resolved\n")], "resolve conflict");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ConflictMarkersChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let got = ids(&report.findings);
        assert_eq!(got, vec!["conflict-markers.history:src/a.rs"]);
        assert_eq!(report.findings[0].severity, Severity::Info);
        assert!(report.findings[0].evidence.contains("add conflict"));
        // 仅 Structure 维度覆盖且被 Info 扣 3 分 → structure = 97。
        assert_eq!(report.scores.structure, Some(97));
    }

    /// 端到端：max_commits=1 限流时窗口外的冲突 blob 不产出结论。
    #[test]
    fn 端到端_限流窗口外冲突不产出() {
        let mut t = TestRepo::init();
        t.commit(
            &[("src/a.rs", conflict_content().as_bytes())],
            "add conflict",
        );
        t.commit(&[("src/a.rs", b"resolved\n")], "resolve conflict");
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(ConflictMarkersChecker)).unwrap();
        let config = ScanConfig {
            max_commits: Some(1),
            ..Default::default()
        };
        let report =
            crate::engine::scan::scan_with_progress(t.path(), &registry, None, None, &config)
                .unwrap();
        assert!(
            report.findings.is_empty(),
            "限流后冲突 blob 属窗口外，不应产出: {:?}",
            ids(&report.findings)
        );
    }
}
