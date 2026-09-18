use serde::{Deserialize, Serialize};

/// 问题严重程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

/// 检查维度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Structure,
    History,
    Branches,
    Deps,
    Security,
}

/// 一条检查结论。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// 稳定唯一标识，如 `history.merge-commit-ratio`。
    pub id: String,
    /// 严重程度。
    pub severity: Severity,
    /// 所属维度。
    pub category: Category,
    /// 简短标题。
    pub title: String,
    /// 证据（触发该结论的数据摘要）。
    pub evidence: String,
    /// 改进建议。
    pub suggestion: String,
}
