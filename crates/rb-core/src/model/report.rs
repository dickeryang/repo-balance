use serde::{Deserialize, Serialize};

use crate::model::finding::Finding;

/// 五维评分（各 0-100，越高越健康）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Scores {
    pub structure: u8,
    pub history: u8,
    pub branches: u8,
    pub deps: u8,
    pub security: u8,
}

impl Scores {
    /// 简单加权总分（各维度均值，u32 中间计算避免 u8 溢出）。
    pub fn total(&self) -> u8 {
        ((self.structure as u32
            + self.history as u32
            + self.branches as u32
            + self.deps as u32
            + self.security as u32)
            / 5) as u8
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
