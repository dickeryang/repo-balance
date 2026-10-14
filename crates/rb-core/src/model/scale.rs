//! 扫描规模预估模型：扫描前对仓库体量与耗时的粗估，供前端提示与自动建议限制档位。

use serde::{Deserialize, Serialize};

/// 仓库规模档位（按提交数划分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleLevel {
    /// 小型：全量扫描毫秒级。
    Small,
    /// 中型：全量扫描秒级。
    Medium,
    /// 大型：建议限制历史深度。
    Large,
    /// 超大型：强烈建议限制历史深度或时间窗。
    Huge,
}

/// 扫描规模预估。
///
/// `estimated_*` 系列为**启发式粗估**（采样最近若干提交的对象数外推全量历史），
/// 不是精确值：仅用于「扫描前给用户对耗时有预期」，不参与任何评分与结论判定。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScaleEstimate {
    /// 本地分支可达提交总数（精确值，来自 revwalk oid 计数）。
    pub commit_count: usize,
    /// 工作区文件数（精确值）。
    pub file_count: usize,
    /// 工作区总字节数（精确值）。
    pub total_bytes: u64,
    /// 采样提交的「每提交树对象数」平均值（0 表示无提交可采样）。
    pub avg_objects_per_commit: f64,
    /// 外推的历史对象总量（= 均值 × 提交数）。
    pub estimated_objects: u64,
    /// 预计历史采集耗时（秒，粗估，最小 1 秒）。
    pub estimated_seconds: u32,
    /// 规模档位。
    pub level: ScaleLevel,
    /// 建议的历史提交数上限（None 表示无需限制）。
    pub suggested_max_commits: Option<usize>,
    /// 建议的历史时间窗（天；None 表示无需限制）。
    pub suggested_since_days: Option<u64>,
}
