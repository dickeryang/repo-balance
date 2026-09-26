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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    ///
    /// # 约定
    /// 必须为**掩码后的证据字符串**，禁止包含未掩码的敏感原文；
    /// 安全类检查的强制掩码规则归属 D3 话术落地（一处掩码，处处掩码：
    /// 序列化、导出与 IPC 复用同一 Finding）。
    pub evidence: String,
    /// 改进建议。
    ///
    /// # 约定
    /// 必须是面向开发者的**可执行中文建议**，包含明确的命令或操作步骤。
    /// 正例：`使用 git filter-repo 从历史中移除该文件`；
    /// 反例：仅"请优化"类无操作步骤的文本。
    pub suggestion: String,
}
