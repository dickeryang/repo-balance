//! 轻量 glob 匹配：`*` 任意字符、`?` 单字符；路径分隔符不特殊处理。
//!
//! 采集层（[`crate::engine::scan`] 的 FileContents 模式过滤）与检查器
//! （自定义规则的路径限定）共用同一口径，避免两处实现漂移。
//! 纯函数、无 I/O、零额外依赖。

/// 路径 glob 匹配：模式为 `all` 时匹配全部；否则匹配完整相对路径或 basename。
///
/// basename 兜底让 `*.rs` 这类无路径分隔符的模式对任意目录深度生效
/// （与 `.gitignore` 的直觉一致）。
pub(crate) fn matches_pattern(pattern: &str, path: &str) -> bool {
    if pattern == "all" {
        return true;
    }
    let basename = path.rsplit('/').next().unwrap_or(path);
    glob_match(pattern, path) || glob_match(pattern, basename)
}

/// 单串 glob 匹配（`*` 任意字符、`?` 单字符；路径分隔符不特殊处理）。
///
/// 经典双指针 + 回溯锚点实现，时间 O(n·m) 最坏、空间 O(1)，不做递归。
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// `all` 模式匹配任意路径（采集层「全部文件」口径）。
    #[test]
    fn all模式_匹配任意路径() {
        assert!(matches_pattern("all", "a/b/c.rs"));
        assert!(matches_pattern("all", ""));
    }

    /// 无分隔符模式按 basename 兜底：任意目录深度的同名文件都命中。
    #[test]
    fn 无分隔符模式_按basename匹配() {
        assert!(matches_pattern("*.rs", "src/deep/mod.rs"));
        assert!(!matches_pattern("*.rs", "src/deep/mod.ts"));
    }

    /// 带分隔符模式按完整路径匹配。
    #[test]
    fn 带分隔符模式_按完整路径匹配() {
        assert!(matches_pattern("src/*.rs", "src/main.rs"));
        assert!(!matches_pattern("src/*.rs", "tests/main.rs"));
    }

    /// `?` 匹配单字符，不匹配空串。
    #[test]
    fn 问号_匹配单字符() {
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(!glob_match("a?c", "abbc"));
    }

    /// `*` 不跨模式内的字面字符：多段通配按顺序匹配。
    #[test]
    fn 星号_按顺序分段匹配() {
        assert!(glob_match("*b*c", "abXc"));
        assert!(!glob_match("*b*c", "acXb"));
    }

    /// 尾部连续 `*` 可匹配空串（模式比文本长也成立）。
    #[test]
    fn 尾部星号_匹配空串() {
        assert!(glob_match("abc*", "abc"));
        assert!(glob_match("abc**", "abc"));
    }
}
