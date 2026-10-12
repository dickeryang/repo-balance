use serde::{Deserialize, Serialize};

use crate::model::finding::Finding;

/// 五维评分（各 0-100，越高越健康；`None` = 该维度无检查器覆盖，显示 N/A）。
///
/// 覆盖度口径：只有注册了对应维度检查器时才计分，避免「无检查器的维度
/// 恒为 100」的假满分误导（roadmap Sprint 1 评分口径修复）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Scores {
    pub structure: Option<u8>,
    pub history: Option<u8>,
    pub branches: Option<u8>,
    pub deps: Option<u8>,
    pub security: Option<u8>,
}

impl Scores {
    /// 总分 = 已覆盖维度的平均分（u32 中间计算避免 u8 溢出）。
    ///
    /// 全部维度未覆盖时返回 `None`（无任何可计分维度）。
    pub fn total(&self) -> Option<u8> {
        let covered = [
            self.structure,
            self.history,
            self.branches,
            self.deps,
            self.security,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<u8>>();
        if covered.is_empty() {
            return None;
        }
        Some((covered.iter().map(|s| *s as u32).sum::<u32>() / covered.len() as u32) as u8)
    }
}

/// 一次扫描的完整报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    /// 被扫描仓库路径。
    pub repo_path: String,
    /// 全部 findings。
    pub findings: Vec<Finding>,
    /// 五维评分。
    pub scores: Scores,
    /// 扫描起始 Unix 时间戳（秒）。
    pub started_at: i64,
    /// 扫描耗时（毫秒）。
    pub duration_ms: u64,
    /// 引擎版本。
    pub engine_version: String,
}