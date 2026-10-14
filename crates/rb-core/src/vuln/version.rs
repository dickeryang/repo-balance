//! 跨生态版本号比较：覆盖 semver 与 Maven / PEP 440 的常见子集。
//!
//! 漏洞匹配只需要回答「版本是否落在 [introduced, fixed) 区间内」，因此这里不做
//! 完整的生态规范实现，只实现各生态**共有的主干语义**：
//!
//! - 主干段按 `.` / `_` / `-` / `+` 切分，数字段数值比较、非数字段字典序比较；
//! - 数字段优先级低于非数字段（semver 规则，npm/crates.io 适用）；
//! - 缺失段按 0 补齐（`1.2` == `1.2.0`）；
//! - 预发布标记（`1.2.3-rc1`、`1.2.3a1`）低于同主干的正式版；
//! - 前缀 `v` / `=` 与构建元数据（`+sha`）忽略。
//!
//! 已知偏差（轻量实现的代价，不追求覆盖全部生态边角）：
//! - Maven 的 `1.2.3.RELEASE` 这类限定符被当作主干段，会略高于 `1.2.3`；
//! - PEP 440 的 `!=`、epoch（`1!2.0`）、`.*` 通配不予支持。

use std::cmp::Ordering;

/// 主干段：数字段与非数字段分开，便于按 semver 优先级比较。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    /// 纯数字段（`1`、`20240101`）。
    Num(u64),
    /// 非数字段（`rc`、`RELEASE`、`beta`）。
    Str(String),
}

/// 解析后的版本号：主干段 + 预发布段。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    main: Vec<Seg>,
    pre: Vec<Seg>,
}

/// 比较两个版本字符串（无法解析的片段按字典序兜底，不 panic）。
pub fn compare(a: &str, b: &str) -> Ordering {
    match (Version::parse(a), Version::parse(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        // 任一侧解析失败时退化为「先按原文比，再按长度比」的稳定序，
        // 保证 compare 是全序（排序与区间判定都不会抖动）。
        _ => a.cmp(b).then_with(|| a.len().cmp(&b.len())),
    }
}

impl Version {
    /// 解析版本字符串；空串或完全无法识别时返回 `None`。
    fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        let s = s
            .strip_prefix('v')
            .or_else(|| s.strip_prefix('V'))
            .unwrap_or(s);
        let s = s.trim_start_matches(['=', '^', '~', '>', '<', ' ']);
        // 构建元数据与空格之后的内容不参与比较。
        let s = s.split(['+', ' ']).next().unwrap_or(s);
        if s.is_empty() {
            return None;
        }

        // 预发布切分：仅当 `-` 后紧跟字母时按 semver 处理（`-rc1`、`-beta.2`）；
        // 形如 `1.0-2` 的连字符版本仍按主干分隔符处理。
        let (main_src, pre_src) = match s.find('-') {
            Some(i)
                if s.as_bytes()
                    .get(i + 1)
                    .is_some_and(|c| c.is_ascii_alphabetic()) =>
            {
                (&s[..i], Some(&s[i + 1..]))
            }
            _ => (s, None),
        };

        let main = split_segments(main_src);
        let mut pre = pre_src.map(split_segments).unwrap_or_default();

        // `1.2.3rc1` / `1.2.3a1`（PEP 440 无分隔符写法）：段内「数字前缀 + 字母后缀」
        // 的字母部分是预发布。纯字母段（Maven 的 `1.2.3.RELEASE`）是限定符而非
        // 预发布，必须保留在主干内，否则会把正式版误判为预发布而低于正式版。
        if pre.is_empty() {
            if let Some(Seg::Str(tail)) = main.last() {
                let peel = tail
                    .starts_with(|c: char| c.is_ascii_digit())
                    .then(|| tail.find(|c: char| c.is_ascii_alphabetic()))
                    .flatten();
                if let Some(i) = peel {
                    let (num, alpha) = tail.split_at(i);
                    let mut m = main.clone();
                    m.pop();
                    m.push(match num.parse::<u64>() {
                        Ok(n) => Seg::Num(n),
                        Err(_) => Seg::Str(num.to_ascii_lowercase()),
                    });
                    pre = vec![Seg::Str(alpha.to_ascii_lowercase())];
                    return Some(Self { main: m, pre });
                }
            }
        }

        if main.is_empty() {
            return None;
        }
        Some(Self { main, pre })
    }
}

/// 按分隔符切分并归一化段序列。
fn split_segments(src: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    for part in src.split(['.', '-', '_']) {
        if part.is_empty() {
            continue;
        }
        out.push(match part.parse::<u64>() {
            Ok(n) => Seg::Num(n),
            Err(_) => Seg::Str(part.to_ascii_lowercase()),
        });
    }
    out
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // 主干逐段比较，缺失段按 Num(0) 补齐。
        let max = self.main.len().max(other.main.len());
        for i in 0..max {
            let a = self.main.get(i).unwrap_or(&Seg::Num(0));
            let b = other.main.get(i).unwrap_or(&Seg::Num(0));
            let ord = cmp_seg(a, b);
            if ord != Ordering::Equal {
                return ord;
            }
        }
        // 主干相等时：带预发布标记的版本更低（semver 规则）。
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                let max = self.pre.len().max(other.pre.len());
                for i in 0..max {
                    let a = self.pre.get(i).unwrap_or(&Seg::Num(0));
                    let b = other.pre.get(i).unwrap_or(&Seg::Num(0));
                    let ord = cmp_seg(a, b);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                Ordering::Equal
            }
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 单段比较：数字 < 非数字，同类内部按数值 / 字典序。
fn cmp_seg(a: &Seg, b: &Seg) -> Ordering {
    match (a, b) {
        (Seg::Num(x), Seg::Num(y)) => x.cmp(y),
        (Seg::Str(x), Seg::Str(y)) => x.cmp(y),
        // semver：数字标识符优先级低于字母标识符。
        (Seg::Num(_), Seg::Str(_)) => Ordering::Less,
        (Seg::Str(_), Seg::Num(_)) => Ordering::Greater,
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::compare;
    use std::cmp::Ordering;

    /// 相等：缺失段按 0 补齐，前缀 v/= 与构建元数据忽略。
    #[test]
    fn 比较_等价写法相等() {
        for (a, b) in [
            ("1.2", "1.2.0"),
            ("v1.2.3", "1.2.3"),
            ("=1.2.3", "1.2.3"),
            ("1.2.3+build.5", "1.2.3"),
            ("1.2.3", "1.2.3 "),
        ] {
            assert_eq!(compare(a, b), Ordering::Equal, "{a} vs {b}");
        }
    }

    /// 主干数值比较：数值而非字典序（否则 "1.10" < "1.9"）。
    #[test]
    fn 比较_数值段按数值而非字典序() {
        assert_eq!(compare("1.9", "1.10"), Ordering::Less);
        assert_eq!(compare("1.0.10", "1.0.9"), Ordering::Greater);
        assert_eq!(compare("2.0", "10.0"), Ordering::Less);
    }

    /// 预发布低于同主干正式版；预发布之间按段比较。
    #[test]
    fn 比较_预发布低于正式版() {
        assert_eq!(compare("1.2.3-rc1", "1.2.3"), Ordering::Less);
        assert_eq!(compare("1.2.3", "1.2.3-beta"), Ordering::Greater);
        assert_eq!(compare("1.2.3-rc1", "1.2.3-rc2"), Ordering::Less);
        assert_eq!(
            compare("1.2.3a1", "1.2.3"),
            Ordering::Less,
            "PEP 440 无分隔符"
        );
        assert_eq!(compare("1.2.3rc1", "1.2.3"), Ordering::Less);
    }

    /// 数字段优先级低于非数字段（semver 规则）。
    #[test]
    fn 比较_数字段低于非数字段() {
        assert_eq!(compare("1.2.3", "1.2.3.RELEASE"), Ordering::Less);
    }

    /// 区间判定语义：introduced / fixed 边界的含与不含。
    #[test]
    fn 比较_区间边界语义() {
        // 命中 [1.0.0, 2.0.0)
        let in_range = |v: &str| {
            compare(v, "1.0.0") != Ordering::Less && compare(v, "2.0.0") == Ordering::Less
        };
        assert!(in_range("1.0.0"), "下界含");
        assert!(in_range("1.9.9"));
        assert!(!in_range("2.0.0"), "上界不含");
        assert!(!in_range("0.9.0"), "下界之前不含");
    }

    /// 全序稳定性：compare 反对称，且排序结果稳定。
    #[test]
    fn 比较_反对称且可排序() {
        let mut versions = vec!["2.0", "1.0.0", "1.0", "1.0.0-rc1", "10.0", "1.10"];
        versions.sort_by(|a, b| compare(a, b));
        // sort_by 稳定：等价写法（1.0 与 1.0.0）保持原相对顺序。
        assert_eq!(
            versions,
            vec!["1.0.0-rc1", "1.0.0", "1.0", "1.10", "2.0", "10.0"]
        );
        for w in windows(&versions) {
            assert_ne!(
                compare(w[1], w[0]),
                Ordering::Less,
                "{} 不应小于 {}",
                w[1],
                w[0]
            );
        }
    }

    /// 取相邻窗口（避免为测试引入 itertools）。
    fn windows<'a>(v: &'a [&'a str]) -> Vec<Vec<&'a str>> {
        v.windows(2).map(|w| w.to_vec()).collect()
    }

    /// 空串等不可解析输入不 panic。
    #[test]
    fn 比较_不可解析输入不panic() {
        for (a, b) in [("", "1.0"), ("1.0", ""), ("", ""), ("latest", "1.0")] {
            let _ = compare(a, b);
        }
    }
}
