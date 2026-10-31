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
        // 四舍五入（非整数截断）：与前端 totalOrNullOf、rb-app 的 HTML/PDF 导出
        // 共用同一口径——整数截断会让 CLI --json 的总分比界面少 1 分（75.5 → 75 vs 76）
        Some(
            (covered.iter().map(|s| *s as u32).sum::<u32>() as f64 / covered.len() as f64).round()
                as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 总分四舍五入而非整数截断，与 GUI / 导出口径一致。
    #[test]
    fn 总分_四舍五入口径() {
        let scores = Scores {
            structure: Some(90),
            history: Some(61),
            branches: None,
            deps: None,
            security: None,
        };
        assert_eq!(scores.total(), Some(76)); // 75.5 → 76（截断会得 75）
        assert_eq!(
            Scores {
                structure: Some(90),
                history: Some(85),
                branches: Some(78),
                deps: Some(40),
                security: None,
            }
            .total(),
            Some(73) // 73.25 → 73
        );
        assert_eq!(
            Scores {
                structure: None,
                history: None,
                branches: None,
                deps: None,
                security: None,
            }
            .total(),
            None
        );
    }
}

/// 单个检查器 `check` 阶段的执行耗时。
///
/// 口径：仅统计纯计算阶段（`Checker::check`）；切片采集与评分耗时不含在内，
/// 因此各条之和通常小于报告级 `duration_ms`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckerTiming {
    /// 检查器 id（与 `Checker::id()` 一致，如 `conflict-markers`）。
    pub checker_id: String,
    /// 执行耗时（毫秒，向下取整，亚毫秒计 0）。
    pub duration_ms: u64,
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
    /// 各检查器 `check` 阶段执行耗时（按注册顺序）。
    pub checker_timings: Vec<CheckerTiming>,
    /// 引擎版本。
    pub engine_version: String,
}
