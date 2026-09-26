//! 扫描上下文与全局配置：Checker 在 plan / check 两阶段共享的只读输入。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::checker::plan::DataSlice;
use crate::model::slice_meta::{BlobMeta, FileMeta};
use crate::model::snapshot::RepoSnapshot;

/// 扫描全局配置载体；具体配置项由各检查器话术逐步追加字段扩展（spec 6.3.3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// 大文件判定阈值（字节），默认 1 MiB（1048576）。
    ///
    /// 注意：手写 [`Default`] 而非 derive——derive 会将 u64 初始化为 0，
    /// 导致默认配置下全仓文件命中大文件判定。
    #[serde(default)]
    pub big_file_threshold: u64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            big_file_threshold: 1_048_576,
        }
    }
}

/// 切片采集结果载荷（封闭枚举，与 [`DataSlice`] 请求一一对应）。
///
/// 载荷均为强类型元数据视图，不含任何文件内容字节（>50MB 零内容读取的
/// 结构级保证）；其余切片（WorkingTree / BranchGraph / DependencyFiles）
/// 的载荷变体归 scan_engine 编排话术后续追加。
#[derive(Debug, Clone)]
pub enum SliceData {
    /// 文件内容切片的元数据视图（FileContents 请求的采集结果）。
    FileContents(Vec<FileMeta>),
    /// 全量历史的 blob 元数据视图（FullHistory 请求的采集结果）。
    FullHistory(Vec<BlobMeta>),
}

/// Checker 在 plan / check 两阶段共享的只读上下文（spec 5.3）。
///
/// 含引用类型，仅 derive Debug（序列化豁免，spec 4.4.2）；不暴露任何
/// 可变通道，检查器无法经此修改快照、切片或配置（spec 4.3.2）。
#[derive(Debug)]
pub struct ScanContext<'a> {
    /// 只读仓库快照。
    snapshot: &'a RepoSnapshot,
    /// 已采集就绪的切片载荷（「已声明未采集」= 不在表中）。
    slices: HashMap<DataSlice, SliceData>,
    /// 只读全局配置。
    config: &'a ScanConfig,
}

impl<'a> ScanContext<'a> {
    /// 构造仅含快照与配置的基础上下文（全部切片未就绪）。
    ///
    /// plan 阶段先于数据采集（两阶段依赖，spec 5.3.2.1），plan 用 base
    /// 上下文即可；check 阶段的完整上下文由编排层注入切片后提供。
    pub fn base(snapshot: &'a RepoSnapshot, config: &'a ScanConfig) -> Self {
        Self {
            snapshot,
            slices: HashMap::new(),
            config,
        }
    }

    /// 注入一个已采集的切片载荷。
    ///
    /// 仅供编排层在采集完成后构建 check 阶段上下文使用（D3 启用）。
    pub fn insert_slice(&mut self, slice: DataSlice, data: SliceData) {
        self.slices.insert(slice, data);
    }

    /// 只读访问仓库快照。
    pub fn snapshot(&self) -> &RepoSnapshot {
        self.snapshot
    }

    /// 只读访问全局配置。
    pub fn config(&self) -> &ScanConfig {
        self.config
    }

    /// 切片是否已采集就绪；false 即「已声明但未采集」状态的唯一表达（spec 5.3.3.1）。
    pub fn is_ready(&self, slice: &DataSlice) -> bool {
        self.slices.contains_key(slice)
    }

    /// 获取切片载荷；未采集时返回 None，检查器不得因此 panic。
    pub fn slice(&self, slice: &DataSlice) -> Option<&SliceData> {
        self.slices.get(slice)
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 内存构造零值快照（不触碰 git）。
    fn snapshot() -> RepoSnapshot {
        RepoSnapshot {
            path: "/tmp/demo".into(),
            branch_count: 0,
            branches: vec![],
            commit_count: 0,
            recent_commits: vec![],
            file_count: 0,
            total_bytes: 0,
            dep_manifests: vec![],
        }
    }

    /// base 构造后对任意切片 is_ready 为 false、slice 为 None。
    #[test]
    fn 基础上下文_全部切片未就绪() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert!(!ctx.is_ready(&DataSlice::FullHistory));
        assert!(ctx.slice(&DataSlice::FullHistory).is_none());
        assert!(!ctx.is_ready(&DataSlice::WorkingTree));
        assert!(ctx.slice(&DataSlice::WorkingTree).is_none());
    }

    /// ScanConfig 默认阈值为 1 MiB（手写 Default 防 0 值坑）。
    #[test]
    fn 默认配置_大文件阈值为1MiB() {
        assert_eq!(ScanConfig::default().big_file_threshold, 1_048_576);
    }

    /// insert_slice 注入后就绪并可取回载荷；未注入切片仍为未就绪。
    #[test]
    fn 注入切片后_就绪且可取回载荷() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let mut ctx = ScanContext::base(&snap, &config);
        ctx.insert_slice(DataSlice::WorkingTree, SliceData::FileContents(vec![]));
        assert!(ctx.is_ready(&DataSlice::WorkingTree));
        assert!(ctx.slice(&DataSlice::WorkingTree).is_some());
        assert!(!ctx.is_ready(&DataSlice::FullHistory));
        assert!(ctx.slice(&DataSlice::FullHistory).is_none());
    }

    /// snapshot() / config() 可只读取用，且不含可变通道。
    #[test]
    fn 只读访问器_可读取快照与配置() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert_eq!(ctx.snapshot().path, "/tmp/demo");
        assert_eq!(ctx.snapshot().commit_count, 0);
        assert!(std::ptr::eq(ctx.config(), &config));
    }
}