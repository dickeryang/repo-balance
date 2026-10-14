//! 离线依赖漏洞检测：把 OSV 风格公告索引与仓库依赖清单做版本匹配。
//!
//! 设计要点：
//! - **纯计算、无 I/O**：索引由壳层（rb-app）读取后注入，清单文本由 `git` 模块采集，
//!   rb-core 自身不碰文件系统（分层纪律）；
//! - **无索引即退化**：索引为空时匹配结果为空，依赖维度回到「仅结构检查」；
//! - **保守优先**：公告缺严重程度按 Warning 处理，宁可提示也不漏报。

pub mod manifest;
pub mod version;

use crate::error::RbError;
use crate::model::vuln::{Advisory, VulnIndex};

pub use manifest::{parse_dep_file, ParsedDep};

/// 索引文件格式：每行一个 JSON 对象（JSONL），字段见 [`Advisory`]。
///
/// 选 JSONL 而非单个 JSON 数组：可流式追加、便于转换脚本增量生成，
/// 也便于用户用文本工具检视。
pub fn load_index(jsonl: &str) -> Result<VulnIndex, RbError> {
    let mut entries = Vec::new();
    for (no, line) in jsonl.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let advisory: Advisory = serde_json::from_str(line)
            .map_err(|e| RbError::VulnDb(format!("第 {} 行解析失败: {e}", no + 1)))?;
        entries.push(advisory);
    }
    Ok(VulnIndex::new(entries))
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::model::vuln::VulnEcosystem;

    /// 正常加载：跳过空行与注释行。
    #[test]
    fn 加载索引_跳过空行与注释() {
        let jsonl = concat!(
            "# generated at 2026-10-14\n",
            "\n",
            r#"{"id":"GHSA-1","eco":"npm","pkg":"lodash","ranges":[{"introduced":"0","fixed":"4.17.21"}]}"#,
            "\n",
            r#"{"id":"RUSTSEC-1","eco":"cargo","pkg":"serde","ranges":[{"introduced":"1.0.0","fixed":"1.0.100"}],"sev":"critical"}"#,
            "\n"
        );
        let idx = load_index(jsonl).unwrap();
        assert_eq!(idx.len(), 2);
        assert_eq!(
            idx.entries()[1].severity(),
            crate::model::finding::Severity::Critical,
            "sev 字段应被解析为 Critical"
        );
        // 缺省 sev 的公告回落 Warning
        assert_eq!(
            idx.entries()[0].severity(),
            crate::model::finding::Severity::Warning
        );
        assert!(idx.matches(VulnEcosystem::Npm, "lodash", "4.17.20").len() == 1);
        assert!(idx
            .matches(VulnEcosystem::Npm, "lodash", "4.17.21")
            .is_empty());
    }

    /// 坏行报错并带行号（便于定位转换脚本产出的问题）。
    #[test]
    fn 加载索引_坏行带行号报错() {
        let err = load_index("{\"id\":\"GHSA-1\",\"eco\":\"npm\",\"pkg\":\"a\"}\nnot-json\n")
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains('2'), "应指出第 2 行: {msg}");
    }

    /// 空输入得到空索引（等价于未接入漏洞库）。
    #[test]
    fn 加载索引_空输入为空库() {
        let idx = load_index("").unwrap();
        assert!(idx.is_empty());
        assert!(idx.matches(VulnEcosystem::Go, "gin", "1.0.0").is_empty());
    }
}
