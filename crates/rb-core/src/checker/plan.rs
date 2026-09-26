//! 数据切片声明与扫描计划：Checker 声明所需数据，编排层统一合并后一次性采集。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::RbError;
use crate::Result;

/// 扫描所需的数据切片（封闭集合，spec 6.2.1）。
///
/// 编排层依据本枚举统一采集数据，避免每个 Checker 重复遍历仓库；
/// 新增变体属于封闭集扩展，需同步编排层采集逻辑。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSlice {
    /// 全量提交历史（HEAD 可达）。
    FullHistory,
    /// 工作区当前文件状态。
    WorkingTree,
    /// 分支拓扑（含合并可达性信息）。
    BranchGraph,
    /// 依赖清单文件的解析结果。
    DependencyFiles,
    /// 文件内容切片，`pattern` 限定范围（如 `*.rs`；`all` 表示全部文件）。
    FileContents {
        /// 限定文件范围的模式。
        pattern: String,
    },
}

impl DataSlice {
    /// 构造文件内容切片；空或纯空白模式在构造入口即被拒绝（spec 5.2.1.2）。
    pub fn file_contents(pattern: &str) -> Result<Self> {
        if pattern.trim().is_empty() {
            return Err(RbError::InvalidSlice(
                "FileContents 模式不能为空或纯空白".into(),
            ));
        }
        Ok(Self::FileContents {
            pattern: pattern.to_owned(),
        })
    }
}

/// 单个检查器的切片采集请求集合（保序）。
///
/// 计划是采集请求的「种类集合」，非采集结果本身；构造时不强制去重，
/// 去重统一收敛于 [`merge_plans`]（同一切片在合并输出中至多一份）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanPlan {
    /// 按声明序保存的切片请求。
    slices: Vec<DataSlice>,
}

impl ScanPlan {
    /// 创建空计划。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个切片请求，链式返回自身。
    pub fn push(&mut self, slice: DataSlice) -> &mut Self {
        self.slices.push(slice);
        self
    }

    /// 已声明的切片清单（按声明序）。
    pub fn slices(&self) -> &[DataSlice] {
        &self.slices
    }

    /// 是否未声明任何切片。
    pub fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }
}

/// 合并多份扫描计划：同一切片只保留一份，输出保持「首次出现序」（spec 6.2.3）。
///
/// 非参数化切片按变体身份去重；`FileContents` 按 `pattern` 精确字符串匹配去重；
/// 任何请求不得丢失（spec 4.1.1 单次采集保证）。纯函数：模式合法性校验归
/// [`DataSlice::file_contents`] 构造入口，本函数不返回 Result。
pub fn merge_plans(plans: &[ScanPlan]) -> ScanPlan {
    let mut seen = HashSet::new();
    let mut merged = ScanPlan::new();
    for plan in plans {
        for slice in plan.slices() {
            if seen.insert(slice.clone()) {
                merged.push(slice.clone());
            }
        }
    }
    merged
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 以切片清单构造计划。
    fn plan_of(slices: &[DataSlice]) -> ScanPlan {
        let mut plan = ScanPlan::new();
        for s in slices {
            plan.push(s.clone());
        }
        plan
    }

    /// N 份计划请求同一切片，合并后仅保留一份（单次采集保证）。
    #[test]
    fn 同一切片多计划请求_合并后仅一份() {
        let plans = vec![
            plan_of(&[DataSlice::FullHistory]),
            plan_of(&[DataSlice::FullHistory, DataSlice::WorkingTree]),
            plan_of(&[DataSlice::FullHistory]),
        ];
        let merged = merge_plans(&plans);
        assert_eq!(
            merged.slices(),
            &[DataSlice::FullHistory, DataSlice::WorkingTree]
        );
    }

    /// 不同切片全部保留，数量与内容一致、请求不丢失。
    #[test]
    fn 不同切片多计划请求_全部保留不丢失() {
        let plans = vec![
            plan_of(&[DataSlice::FullHistory, DataSlice::BranchGraph]),
            plan_of(&[DataSlice::DependencyFiles, DataSlice::WorkingTree]),
        ];
        let merged = merge_plans(&plans);
        assert_eq!(merged.slices().len(), 4);
        for s in [
            DataSlice::FullHistory,
            DataSlice::BranchGraph,
            DataSlice::DependencyFiles,
            DataSlice::WorkingTree,
        ] {
            assert!(merged.slices().contains(&s), "请求 {s:?} 丢失");
        }
    }

    /// FileContents 按 pattern 精确匹配去重：同模式合并为一份，异模式各自保留。
    #[test]
    fn 文件内容切片_同模式精确去重_异模式保留() {
        let same = DataSlice::FileContents {
            pattern: "*.rs".into(),
        };
        let other = DataSlice::FileContents {
            pattern: "*.toml".into(),
        };
        let plans = vec![
            plan_of(&[same.clone(), other.clone()]),
            plan_of(std::slice::from_ref(&same)),
        ];
        let merged = merge_plans(&plans);
        assert_eq!(merged.slices(), &[same, other]);
    }

    /// 空计划合并：不新增条目、不影响其他计划的请求。
    #[test]
    fn 空计划合并_不新增条目不影响他计划() {
        let plans = vec![ScanPlan::new(), plan_of(&[DataSlice::FullHistory])];
        let merged = merge_plans(&plans);
        assert_eq!(merged.slices(), &[DataSlice::FullHistory]);
    }

    /// DataSlice 与 ScanPlan 的 JSON 序列化往返等价，字段名 snake_case 稳定。
    #[test]
    fn 数据切片与计划_JSON往返等价_snake_case稳定() {
        let plan = plan_of(&[
            DataSlice::FullHistory,
            DataSlice::file_contents("*.rs").unwrap(),
        ]);
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("full_history"), "{json}");
        assert!(json.contains("file_contents"), "{json}");
        assert!(json.contains("pattern"), "{json}");
        let back: ScanPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    /// 空模式与纯空白模式在构造入口即被拒绝。
    #[test]
    fn 文件内容切片_空或纯空白模式_构造即拒绝() {
        assert!(DataSlice::file_contents("").is_err());
        assert!(DataSlice::file_contents("   ").is_err());
        assert!(DataSlice::file_contents("\t\n").is_err());
        assert!(DataSlice::file_contents("*.rs").is_ok());
    }
}