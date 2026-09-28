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
        let mut patterns = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
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
            patterns.push(Pattern {
                negate,
                anchored,
                dir_only,
                segments,
            });
        }
        Self { patterns }
    }

    /// 从仓库根目录读取 `.repobalance-ignore` 并解析；文件不存在时返回空规则。
    pub fn load_from(workdir: &std::path::Path) -> Self {
        let path = workdir.join(".repobalance-ignore");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
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
    use super::IgnoreRule;

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
}
