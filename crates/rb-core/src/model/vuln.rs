//! 离线漏洞库模型：安全公告的紧凑表示，供依赖维度做版本匹配。
//!
//! 数据由壳层（rb-app）从应用数据目录的 JSONL 索引读入后注入扫描上下文；
//! rb-core 自身不做任何文件 I/O（分层纪律：文件读取一律经 `git` 模块）。
//! 索引缺失时依赖维度退化为「仅结构检查」，与接入漏洞库前的行为一致。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::finding::Severity;
use crate::model::slice_meta::Ecosystem;
use crate::vuln::version::compare;

/// 漏洞库生态（索引内使用小写短名，转换脚本负责与 OSV 生态名对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VulnEcosystem {
    /// crates.io（Rust）。
    Cargo,
    /// npm（JavaScript）。
    Npm,
    /// PyPI（Python）。
    Pypi,
    /// Go modules。
    Go,
    /// Maven（Java）。
    Maven,
}

impl VulnEcosystem {
    /// 全部受支持生态（封闭集）。
    pub const ALL: [VulnEcosystem; 5] = [
        VulnEcosystem::Cargo,
        VulnEcosystem::Npm,
        VulnEcosystem::Pypi,
        VulnEcosystem::Go,
        VulnEcosystem::Maven,
    ];

    /// 由依赖文件生态映射到漏洞库生态。
    ///
    /// Ruby / PHP 暂无对应索引（转换脚本未收录），返回 `None` 表示跳过匹配。
    pub fn from_ecosystem(eco: Ecosystem) -> Option<Self> {
        match eco {
            Ecosystem::Rust => Some(Self::Cargo),
            Ecosystem::JavaScript => Some(Self::Npm),
            Ecosystem::Python => Some(Self::Pypi),
            Ecosystem::Go => Some(Self::Go),
            Ecosystem::Java => Some(Self::Maven),
            Ecosystem::Ruby | Ecosystem::Php => None,
        }
    }

    /// 序列化后的生态短名（与 JSONL 索引一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Pypi => "pypi",
            Self::Go => "go",
            Self::Maven => "maven",
        }
    }

    /// 界面展示用的中文生态名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Cargo => "Rust",
            Self::Npm => "npm",
            Self::Pypi => "PyPI",
            Self::Go => "Go",
            Self::Maven => "Maven",
        }
    }
}

/// 单个受影响版本区间（OSV `affected[].ranges[].events` 的常用子集）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulnRange {
    /// 起始版本（含）；`None` 表示「自首个版本起」。
    #[serde(default)]
    pub introduced: Option<String>,
    /// 修复版本（不含）；`None` 表示该区间无修复版本（与 `last_affected` 配合）。
    #[serde(default)]
    pub fixed: Option<String>,
    /// 最后一个受影响版本（含）；`None` 表示以 `fixed` 界定。
    #[serde(default)]
    pub last_affected: Option<String>,
}

impl VulnRange {
    /// 判断 `version` 是否落在该区间内。
    ///
    /// 语义：`version >= introduced`（缺省则不限下界）
    /// 且 `version < fixed`（缺省则不限上界）
    /// 且 `version <= last_affected`（缺省则不限）。
    pub fn contains(&self, version: &str) -> bool {
        if let Some(introduced) = &self.introduced {
            if compare(version, introduced) == std::cmp::Ordering::Less {
                return false;
            }
        }
        if let Some(fixed) = &self.fixed {
            if compare(version, fixed) != std::cmp::Ordering::Less {
                return false;
            }
        }
        if let Some(last) = &self.last_affected {
            if compare(version, last) == std::cmp::Ordering::Greater {
                return false;
            }
        }
        true
    }

    /// 区间的中文描述（用于 evidence）。
    pub fn describe(&self) -> String {
        match (&self.introduced, &self.fixed, &self.last_affected) {
            (Some(i), Some(f), _) => format!("[{i}, {f})"),
            (Some(i), None, Some(l)) => format!("[{i}, {l}]"),
            (None, Some(f), _) => format!("< {f}"),
            (None, None, Some(l)) => format!("<= {l}"),
            _ => "全部版本".to_owned(),
        }
    }
}

/// 一条安全公告（紧凑表示，仅保留匹配与展示所需字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advisory {
    /// 公告 id（`GHSA-*` / `RUSTSEC-*` / `CVE-*` 等）。
    pub id: String,
    /// 生态。
    pub eco: VulnEcosystem,
    /// 包名（已归一化为小写）。
    pub pkg: String,
    /// 受影响区间集合；命中任一区间即命中（区间为空时视为全版本受影响）。
    #[serde(default)]
    pub ranges: Vec<VulnRange>,
    /// 严重程度；缺失时按 [`Severity::Warning`] 处理（宁可提示也不漏报）。
    #[serde(default)]
    pub sev: Option<Severity>,
    /// 一句话摘要（上游公告原文，可能为英文）。
    #[serde(default)]
    pub summary: String,
}

impl Advisory {
    /// 命中判定：区间为空视为全版本受影响，否则命中任一区间即命中。
    pub fn affects(&self, version: &str) -> bool {
        self.ranges.is_empty() || self.ranges.iter().any(|r| r.contains(version))
    }

    /// 对外展示的严重程度（缺失回落 Warning）。
    pub fn severity(&self) -> Severity {
        self.sev.unwrap_or(Severity::Warning)
    }

    /// 归一化包名（小写、去空白）；npm 与 crates.io 均按小写比较。
    pub fn normalize_pkg(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }
}

/// 离线漏洞库索引：公告集合 + (生态, 包名) 倒排表。
#[derive(Debug, Clone, Default)]
pub struct VulnIndex {
    entries: Vec<Advisory>,
    /// (生态, 归一化包名) → `entries` 下标集合。
    by_key: HashMap<(VulnEcosystem, String), Vec<usize>>,
}

impl VulnIndex {
    /// 由公告集合构建索引（包名在构建时归一化，匹配端不再重复小写化）。
    pub fn new(entries: Vec<Advisory>) -> Self {
        let mut by_key: HashMap<(VulnEcosystem, String), Vec<usize>> = HashMap::new();
        for (i, a) in entries.iter().enumerate() {
            by_key
                .entry((a.eco, Advisory::normalize_pkg(&a.pkg)))
                .or_default()
                .push(i);
        }
        Self { entries, by_key }
    }

    /// 公告条数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空（空库等价于未接入，依赖维度退化为结构检查）。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 全部公告（按索引内顺序）。
    pub fn entries(&self) -> &[Advisory] {
        &self.entries
    }

    /// 查询某依赖命中的全部公告（按 id 排序，保证输出稳定）。
    pub fn matches(&self, eco: VulnEcosystem, pkg: &str, version: &str) -> Vec<&Advisory> {
        let Some(idx) = self.by_key.get(&(eco, Advisory::normalize_pkg(pkg))) else {
            return Vec::new();
        };
        let mut hits: Vec<&Advisory> = idx
            .iter()
            .map(|i| &self.entries[*i])
            .filter(|a| a.affects(version))
            .collect();
        hits.sort_by(|a, b| a.id.cmp(&b.id));
        hits
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn range(introduced: &str, fixed: &str) -> VulnRange {
        VulnRange {
            introduced: Some(introduced.into()),
            fixed: Some(fixed.into()),
            last_affected: None,
        }
    }

    fn adv(id: &str, eco: VulnEcosystem, pkg: &str, ranges: Vec<VulnRange>) -> Advisory {
        Advisory {
            id: id.into(),
            eco,
            pkg: pkg.into(),
            ranges,
            sev: None,
            summary: String::new(),
        }
    }

    /// 区间语义：含下界、不含上界。
    #[test]
    fn 区间_含下界不含上界() {
        let r = range("1.0.0", "2.0.0");
        assert!(r.contains("1.0.0"));
        assert!(r.contains("1.9.9"));
        assert!(!r.contains("2.0.0"));
        assert!(!r.contains("0.9.9"));
    }

    /// 区间描述文案（evidence 用）。
    #[test]
    fn 区间_描述文案() {
        assert_eq!(range("1.0.0", "2.0.0").describe(), "[1.0.0, 2.0.0)");
        assert_eq!(
            VulnRange {
                introduced: None,
                fixed: Some("3.0".into()),
                last_affected: None
            }
            .describe(),
            "< 3.0"
        );
        assert_eq!(
            VulnRange {
                introduced: None,
                fixed: None,
                last_affected: None
            }
            .describe(),
            "全部版本"
        );
    }

    /// 无区间视为全版本受影响（区间缺失不得导致漏报）。
    #[test]
    fn 公告_无区间视为全版本受影响() {
        let a = adv("GHSA-x", VulnEcosystem::Npm, "lodash", vec![]);
        assert!(a.affects("0.0.1"));
        assert!(a.affects("99.0.0"));
    }

    /// 严重程度缺失回落 Warning。
    #[test]
    fn 公告_严重程度缺失回落Warning() {
        assert_eq!(
            adv("a", VulnEcosystem::Go, "x", vec![]).severity(),
            Severity::Warning
        );
    }

    /// 索引匹配：按生态 + 包名定位，包名大小写不敏感。
    #[test]
    fn 索引_按生态包名匹配且忽略大小写() {
        let idx = VulnIndex::new(vec![
            adv(
                "GHSA-1",
                VulnEcosystem::Npm,
                "Lodash",
                vec![range("0", "4.17.21")],
            ),
            adv(
                "GHSA-2",
                VulnEcosystem::Cargo,
                "serde",
                vec![range("1.0.0", "1.0.100")],
            ),
        ]);
        assert_eq!(idx.len(), 2);
        let hits = idx.matches(VulnEcosystem::Npm, "lodash", "4.17.20");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "GHSA-1");
        // 已修复版本不应命中
        assert!(idx
            .matches(VulnEcosystem::Npm, "lodash", "4.17.21")
            .is_empty());
        // 生态不隔离会误报：npm 包不会被 cargo 公告命中
        assert!(idx
            .matches(VulnEcosystem::Cargo, "lodash", "4.17.20")
            .is_empty());
    }

    /// 未收录的包名直接返回空（大索引下的快路径）。
    #[test]
    fn 索引_未收录包名返回空() {
        let idx = VulnIndex::new(vec![adv("GHSA-1", VulnEcosystem::Go, "gin", vec![])]);
        assert!(idx.matches(VulnEcosystem::Go, "echo", "1.0.0").is_empty());
        assert!(!idx.is_empty());
        assert!(VulnIndex::default().is_empty());
    }

    /// 生态映射：不支持的生态（Ruby / PHP）返回 None。
    #[test]
    fn 生态映射_不支持的生态返回None() {
        assert_eq!(
            VulnEcosystem::from_ecosystem(Ecosystem::Rust),
            Some(VulnEcosystem::Cargo)
        );
        assert_eq!(
            VulnEcosystem::from_ecosystem(Ecosystem::JavaScript),
            Some(VulnEcosystem::Npm)
        );
        assert_eq!(VulnEcosystem::from_ecosystem(Ecosystem::Ruby), None);
        assert_eq!(VulnEcosystem::from_ecosystem(Ecosystem::Php), None);
    }

    /// 生态短名与序列化一致（JSONL 索引契约）。
    #[test]
    fn 生态短名_与序列化一致() {
        for eco in VulnEcosystem::ALL {
            let json = serde_json::to_string(&eco).unwrap();
            assert_eq!(json, format!("\"{}\"", eco.as_str()), "{eco:?}");
        }
    }

    /// 公告 JSON 往返与缺省字段兼容（ranges / sev / summary 可缺省）。
    #[test]
    fn 公告_JSON往返与缺省字段() {
        let a: Advisory =
            serde_json::from_str(r#"{"id":"RUSTSEC-1","eco":"cargo","pkg":"foo"}"#).unwrap();
        assert!(a.ranges.is_empty());
        assert_eq!(a.sev, None);
        assert_eq!(a.summary, "");
        let back = serde_json::to_string(&a).unwrap();
        assert!(back.contains("\"eco\":\"cargo\""), "{back}");
    }
}
