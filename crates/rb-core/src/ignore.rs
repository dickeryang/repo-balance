//! 忽略规则：解析 `.repobalance-ignore` 文件，支持 gitignore 风格子集。
//!
//! 支持语法：
//! - 空行与 `#` 开头注释跳过
//! - `!` 前缀表示否定（取消忽略）
//! - 前导 `/` 锚定到仓库根
//! - 尾随 `/` 仅匹配目录（即匹配该目录下所有文件）
//! - `*` 匹配除 `/` 外任意字符
//! - `?` 匹配单个非 `/` 字符
//! - `**` 匹配任意层级路径段
//!
//! 规则按声明顺序求值，最后匹配的规则决定结果（与 gitignore 语义一致）。
//!
//! # 内置默认忽略
//! [`IgnoreRule::load_from`] 会在用户规则之前注入构建产物/依赖目录默认项
//! （`target/`、`node_modules/` 等）：用户规则声明在后因此优先，
//! 可用 `!` 否定取消默认忽略（如 `!target/keep.bin`）。

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 忽略规则文件名（位于仓库根目录）。
pub const IGNORE_FILE_NAME: &str = ".repobalance-ignore";

/// 写入时的临时文件名（原子写：先落临时文件再 rename）。
const IGNORE_TMP_FILE_NAME: &str = ".repobalance-ignore.tmp";

/// 内置默认忽略的目录模式（构建产物 / 依赖缓存，非仓库源资产）。
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "target/",
    "node_modules/",
    "dist/",
    "build/",
    "out/",
    ".venv/",
    "venv/",
    "__pycache__/",
    ".pytest_cache/",
    ".gradle/",
    ".next/",
    ".nuxt/",
    "coverage/",
];

/// 忽略规则集。
#[derive(Debug, Clone, Default)]
pub struct IgnoreRule {
    patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
struct Pattern {
    negate: bool,
    anchored: bool,
    dir_only: bool,
    segments: Vec<String>,
}

impl IgnoreRule {
    /// 从文件内容文本解析规则集。
    pub fn parse(text: &str) -> Self {
        let patterns = text.lines().filter_map(parse_line).collect();
        Self { patterns }
    }

    /// 解析用户规则并在其**之前**注入内置默认忽略（用户规则声明在后因此优先）。
    ///
    /// 供 GUI 编辑器试算使用：尚未保存的规则必须按「保存后的真实口径」求值，
    /// 否则试算结论会与扫描结论不一致（试算说忽略、扫描照旧报警）。
    pub fn parse_with_defaults(text: &str) -> Self {
        let mut rule = Self::parse(text);
        let defaults: Vec<Pattern> = DEFAULT_IGNORE_PATTERNS
            .iter()
            .filter_map(|p| parse_line(p))
            .collect();
        rule.patterns.splice(0..0, defaults);
        rule
    }

    /// 从仓库根目录读取 `.repobalance-ignore` 并解析（含内置默认忽略）。
    ///
    /// 文件不存在 / 读取失败时退化为「仅内置默认忽略」。
    pub fn load_from(workdir: &Path) -> Self {
        let path = workdir.join(IGNORE_FILE_NAME);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        Self::parse_with_defaults(&text)
    }

    /// 原子写入 `.repobalance-ignore`（先落临时文件再 rename，避免半截写入）。
    ///
    /// 写入的是**纯用户规则**：内置默认忽略由 [`Self::parse_with_defaults`] 在
    /// 载入时注入，不写进文件（否则用户无法用 `!` 干净地否定某一项）。
    pub fn save_to(workdir: &Path, text: &str) -> std::io::Result<()> {
        let tmp = workdir.join(IGNORE_TMP_FILE_NAME);
        if let Err(e) = std::fs::write(&tmp, text) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        match std::fs::rename(&tmp, workdir.join(IGNORE_FILE_NAME)) {
            Ok(()) => Ok(()),
            Err(e) => {
                // rename 失败时清理临时文件，错误以 rename 为准返回。
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    /// 判断路径是否被忽略（正斜杠相对路径）。
    pub fn is_ignored(&self, path: &str) -> bool {
        let path_segments: Vec<&str> = path.split('/').collect();
        let mut ignored = false;
        for pat in &self.patterns {
            if pat.matches(&path_segments) {
                ignored = !pat.negate;
            }
        }
        ignored
    }
}

/// 内置默认忽略模式（只读）：供 GUI 展示「哪些目录默认就被忽略」。
pub fn default_patterns() -> &'static [&'static str] {
    DEFAULT_IGNORE_PATTERNS
}

/// 规则静态检查的问题类别。
///
/// 只描述类别、不携带界面文案：文案由 GUI 按 locale 渲染（内核不写死语言）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IgnoreLintKind {
    /// 模式不含有效路径段（如 `/`、`!`），永不匹配任何文件。
    EmptyPattern,
    /// 用反斜杠分隔路径（引擎按 `/` 分段，这类规则不会匹配到任何文件）。
    Backslash,
    /// 与前面某条规则完全相同：按「最后匹配胜出」语义，前面那条无实际效果。
    Duplicate,
}

/// 单条规则的静态检查结论。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IgnoreLint {
    /// 行号（从 1 计，与编辑器行号一致）。
    pub line: usize,
    /// 该行 trim 后的原始模式。
    pub pattern: String,
    /// 问题类别。
    pub kind: IgnoreLintKind,
}

/// 静态检查**用户规则**文本，返回可疑行（按行号升序；内置默认忽略不参与）。
///
/// 检查项都是「写了却不会生效」的静默情况：用户以为忽略了某路径，扫描照旧
/// 报警——这比直接报错更难发现，所以要在保存前就提示。
pub fn lint(text: &str) -> Vec<IgnoreLint> {
    fn push(out: &mut Vec<IgnoreLint>, line: usize, pattern: &str, kind: IgnoreLintKind) {
        out.push(IgnoreLint {
            line,
            pattern: pattern.to_owned(),
            kind,
        });
    }

    let mut out = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for (idx, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let line = idx + 1;
        if let Some(pat) = parse_line(trimmed) {
            if pat.segments.is_empty() {
                push(&mut out, line, trimmed, IgnoreLintKind::EmptyPattern);
            }
        }
        if trimmed.contains('\\') {
            push(&mut out, line, trimmed, IgnoreLintKind::Backslash);
        }
        if !seen.insert(trimmed) {
            push(&mut out, line, trimmed, IgnoreLintKind::Duplicate);
        }
    }
    out
}

impl Pattern {
    fn matches(&self, path_segments: &[&str]) -> bool {
        if self.segments.is_empty() {
            return false;
        }
        if self.dir_only {
            return self.matches_prefix(path_segments);
        }
        if self.anchored {
            return glob_match_segments(&self.segments, path_segments);
        }
        for start in 0..path_segments.len() {
            if glob_match_segments(&self.segments, &path_segments[start..]) {
                return true;
            }
        }
        false
    }

    fn matches_prefix(&self, path_segments: &[&str]) -> bool {
        if self.anchored {
            return path_segments.len() > self.segments.len()
                && glob_match_segments(&self.segments, &path_segments[..self.segments.len()]);
        }
        for start in 0..path_segments.len() {
            let end = start + self.segments.len();
            if end < path_segments.len()
                && glob_match_segments(&self.segments, &path_segments[start..end])
            {
                return true;
            }
        }
        false
    }
}

/// 解析单行规则；空行与注释返回 None。
fn parse_line(line: &str) -> Option<Pattern> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (negate, rest) = if let Some(stripped) = trimmed.strip_prefix('!') {
        (true, stripped)
    } else {
        (false, trimmed)
    };
    let dir_only = rest.ends_with('/');
    let body = rest.trim_end_matches('/');
    let anchored = body.starts_with('/');
    let body = body.trim_start_matches('/');
    let segments: Vec<String> = if body.is_empty() {
        Vec::new()
    } else {
        body.split('/').map(|s| s.to_owned()).collect()
    };
    Some(Pattern {
        negate,
        anchored,
        dir_only,
        segments,
    })
}

fn glob_match_segments(pat_segs: &[String], path_segs: &[&str]) -> bool {
    let mut pi = 0;
    let mut si = 0;
    let mut star_pi: Option<usize> = None;
    let mut star_si: usize = 0;
    while si < path_segs.len() {
        if pi < pat_segs.len() && pat_segs[pi] == "**" {
            star_pi = Some(pi);
            star_si = si;
            pi += 1;
            continue;
        }
        if pi < pat_segs.len() && glob_match_single(&pat_segs[pi], path_segs[si]) {
            pi += 1;
            si += 1;
            continue;
        }
        if let Some(sp) = star_pi {
            pi = sp + 1;
            star_si += 1;
            si = star_si;
            continue;
        }
        return false;
    }
    while pi < pat_segs.len() && pat_segs[pi] == "**" {
        pi += 1;
    }
    pi == pat_segs.len()
}

fn glob_match_single(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star_p: Option<usize> = None;
    let mut star_t: usize = 0;
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{
        default_patterns, lint, IgnoreLintKind, IgnoreRule, IGNORE_FILE_NAME, IGNORE_TMP_FILE_NAME,
    };

    #[test]
    fn 空内容_不忽略任何路径() {
        let rule = IgnoreRule::parse("");
        assert!(!rule.is_ignored("a.txt"));
    }

    #[test]
    fn 注释与空行_跳过() {
        let rule = IgnoreRule::parse("# comment\n\n  \n*.bin");
        assert!(rule.is_ignored("a.bin"));
        assert!(!rule.is_ignored("a.txt"));
    }

    #[test]
    fn 通配符_匹配任意层级() {
        let rule = IgnoreRule::parse("*.bin");
        assert!(rule.is_ignored("a.bin"));
        assert!(rule.is_ignored("sub/a.bin"));
        assert!(rule.is_ignored("deep/nested/a.bin"));
        assert!(!rule.is_ignored("a.txt"));
    }

    #[test]
    fn 前导斜杠_锚定根() {
        let rule = IgnoreRule::parse("/vendor");
        assert!(rule.is_ignored("vendor"));
        assert!(!rule.is_ignored("sub/vendor"));
    }

    #[test]
    fn 尾随斜杠_匹配目录下所有文件() {
        let rule = IgnoreRule::parse("node_modules/");
        assert!(!rule.is_ignored("node_modules"));
        assert!(rule.is_ignored("node_modules/a.js"));
        assert!(rule.is_ignored("node_modules/sub/b.js"));
        assert!(rule.is_ignored("sub/node_modules/a.js"));
    }

    #[test]
    fn 锚定目录_仅根目录下() {
        let rule = IgnoreRule::parse("/build/");
        assert!(rule.is_ignored("build/output.o"));
        assert!(!rule.is_ignored("sub/build/output.o"));
    }

    #[test]
    fn 否定规则_取消忽略() {
        let rule = IgnoreRule::parse("*.bin\n!important.bin");
        assert!(rule.is_ignored("a.bin"));
        assert!(!rule.is_ignored("important.bin"));
    }

    #[test]
    fn 问号通配_匹配单字符() {
        let rule = IgnoreRule::parse("?.txt");
        assert!(rule.is_ignored("a.txt"));
        assert!(rule.is_ignored("b.txt"));
        assert!(!rule.is_ignored("ab.txt"));
    }

    #[test]
    fn 双星号_跨层级匹配() {
        let rule = IgnoreRule::parse("src/**/test.txt");
        assert!(rule.is_ignored("src/test.txt"));
        assert!(rule.is_ignored("src/a/test.txt"));
        assert!(rule.is_ignored("src/a/b/test.txt"));
        assert!(!rule.is_ignored("other/test.txt"));
    }

    #[test]
    fn 最后匹配规则胜出() {
        let rule = IgnoreRule::parse("*.log\n!keep.log\n*.log");
        assert!(rule.is_ignored("a.log"));
        assert!(rule.is_ignored("keep.log"));
    }

    #[test]
    fn load_from_文件不存在_返回空规则() {
        let dir = std::env::temp_dir();
        let rule = IgnoreRule::load_from(&dir);
        assert!(!rule.is_ignored("anything.txt"));
    }

    /// load_from 注入内置默认忽略：构建产物/依赖目录默认被忽略。
    #[test]
    fn load_from_注入内置默认忽略() {
        let dir = std::env::temp_dir();
        let rule = IgnoreRule::load_from(&dir);
        assert!(rule.is_ignored("target/debug/rb"));
        assert!(rule.is_ignored("ui/node_modules/a.js"));
        assert!(rule.is_ignored("deep/nested/__pycache__/x.pyc"));
        assert!(
            !rule.is_ignored("src/main.rs"),
            "普通源文件不受默认忽略影响"
        );
    }

    /// 用户规则声明在后，可用 `!` 否定内置默认忽略。
    #[test]
    fn 内置默认忽略_用户否定规则优先() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".repobalance-ignore"), "!target/keep.bin\n").unwrap();
        let rule = IgnoreRule::load_from(dir.path());
        assert!(rule.is_ignored("target/debug/rb"), "默认忽略仍生效");
        assert!(!rule.is_ignored("target/keep.bin"), "用户否定规则优先");
    }

    /// 显式 parse 不注入内置默认（仅 load_from 注入），保持纯函数语义。
    #[test]
    fn parse_显式解析_不含内置默认() {
        let rule = IgnoreRule::parse("");
        assert!(!rule.is_ignored("target/debug/rb"));
    }

    #[test]
    fn load_from_读取并解析() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".repobalance-ignore"),
            "*.bin\n# comment\n/vendor/\n",
        )
        .unwrap();
        let rule = IgnoreRule::load_from(dir.path());
        assert!(rule.is_ignored("a.bin"));
        assert!(rule.is_ignored("vendor/lib.a"));
        assert!(!rule.is_ignored("a.txt"));
    }

    /// 试算口径必须与扫描口径一致：编辑器里未保存的文本按「保存后」求值。
    #[test]
    fn parse_with_defaults_与_load_from_口径一致() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(IGNORE_FILE_NAME),
            "*.bin\n!target/keep.bin\n",
        )
        .unwrap();
        let from_file = IgnoreRule::load_from(dir.path());
        let from_text = IgnoreRule::parse_with_defaults("*.bin\n!target/keep.bin\n");
        for path in [
            "a.bin",
            "sub/a.bin",
            "target/debug/rb",
            "target/keep.bin",
            "src/main.rs",
        ] {
            assert_eq!(
                from_file.is_ignored(path),
                from_text.is_ignored(path),
                "路径 {path} 的试算口径与扫描口径不一致"
            );
        }
    }

    /// 保存 → 读回的往返：内容与结论都保持（含 `!` 否定内置默认）。
    #[test]
    fn save_to_写入后_load_from_可读回() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            !dir.path().join(IGNORE_FILE_NAME).exists(),
            "初始不应存在忽略文件"
        );
        IgnoreRule::save_to(dir.path(), "*.bin\n!target/keep.bin\n").unwrap();
        assert!(
            !dir.path().join(IGNORE_TMP_FILE_NAME).exists(),
            "原子写不应残留临时文件"
        );
        let text = std::fs::read_to_string(dir.path().join(IGNORE_FILE_NAME)).unwrap();
        assert_eq!(text, "*.bin\n!target/keep.bin\n");
        let rule = IgnoreRule::load_from(dir.path());
        assert!(rule.is_ignored("a.bin"));
        assert!(rule.is_ignored("target/debug/rb"), "内置默认仍生效");
        assert!(!rule.is_ignored("target/keep.bin"), "用户否定规则优先");
    }

    #[test]
    fn default_patterns_含构建产物目录() {
        let defaults = default_patterns();
        assert!(defaults.contains(&"target/"));
        assert!(defaults.contains(&"node_modules/"));
    }

    /// 空模式（`/`、`!`）解析后无路径段，永不匹配——必须被静态检查标出。
    #[test]
    fn lint_空模式被标出() {
        let lints = lint("# 注释\n\n/\n!\n*.bin\n");
        assert_eq!(lints.len(), 2, "{lints:?}");
        assert_eq!(lints[0].line, 3);
        assert_eq!(lints[0].kind, IgnoreLintKind::EmptyPattern);
        assert_eq!(lints[1].line, 4);
        assert!(lints.iter().all(|l| l.kind == IgnoreLintKind::EmptyPattern));
    }

    #[test]
    fn lint_反斜杠规则被标出() {
        let lints = lint("src\\dist\n");
        assert_eq!(lints.len(), 1, "{lints:?}");
        assert_eq!(lints[0].kind, IgnoreLintKind::Backslash);
        assert_eq!(lints[0].pattern, "src\\dist");
    }

    /// 重复规则只标第二次及以后（第一次出现是正常的）。
    #[test]
    fn lint_重复规则标后一条() {
        let lints = lint("*.bin\n# 说明\n*.bin\n*.bin\ndocs/\n");
        assert_eq!(lints.len(), 2, "{lints:?}");
        assert_eq!(
            lints.iter().map(|l| l.line).collect::<Vec<_>>(),
            vec![3, 4],
            "重复按行号升序标出"
        );
        assert!(lints.iter().all(|l| l.kind == IgnoreLintKind::Duplicate));
    }

    #[test]
    fn lint_正常规则无告警() {
        let lints = lint("# 构建产物\n*.bin\n/dist/\n!keep.bin\n\n");
        assert!(lints.is_empty(), "{lints:?}");
    }
}
