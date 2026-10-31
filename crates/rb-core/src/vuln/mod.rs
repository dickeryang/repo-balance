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
    let mut bad_lines = 0usize;
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // 坏行跳过而非整库失败：一行手改坏不该废掉整个漏洞库（那样依赖维度
        // 会静默退化为「不查漏洞」，而状态页仍显示「已载入」）；全坏才报错。
        match serde_json::from_str::<Advisory>(line) {
            Ok(advisory) => entries.push(advisory),
            Err(_) => bad_lines += 1,
        }
    }
    if entries.is_empty() {
        return Err(RbError::VulnDb(if bad_lines > 0 {
            format!("漏洞库索引无有效条目（{bad_lines} 行均无法解析）")
        } else {
            "漏洞库索引为空（无有效条目）".to_owned()
        }));
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

    /// 坏行跳过而非整库失效（一行手改坏不该废掉整个漏洞库）；全部无效才报错。
    #[test]
    fn 加载索引_坏行跳过_全坏报错() {
        // 1 好行 + 1 坏行：好行保留，漏洞匹配能力不因单行损坏丢失
        let index = load_index("{\"id\":\"GHSA-1\",\"eco\":\"npm\",\"pkg\":\"a\"}\nnot-json\n")
            .expect("含坏行的库不应整体失败");
        assert!(!index.is_empty(), "好行应被保留");

        // 全部无效：报错（先验后写据此拒绝导入）
        let err = load_index("garbage\nalso-bad\n").unwrap_err();
        assert!(err.to_string().contains("2 行"), "{err}");

        // 空库：报错
        assert!(load_index("\n# 注释\n").is_err());
    }

    /// 空输入报错而非空索引：调用方（壳层）对「文件不存在」与「文件为空」
    /// 的处理都是不接入漏洞库，无需用空索引区分。
    #[test]
    fn 加载索引_空输入报错() {
        assert!(load_index("").is_err());
    }
}
