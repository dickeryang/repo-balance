//! 扫描编排：开仓 → 快照 → 收集切片计划 → 合并去重 → 定点采集 → 注入上下文 → 逐检查器 check → 聚合 findings → 报告。
use std::time::Instant;

use crate::checker::{
    merge_plans, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData,
};
use crate::git::GitRepo;
use crate::model::report::{ScanReport, Scores};
use crate::Result;

/// 历史对象遍历的默认批次大小。
const DEFAULT_BATCH_SIZE: usize = 64;

/// 对 `path` 执行完整扫描。
///
/// 签名 `scan(&Path, &CheckerRegistry) -> Result<ScanReport>` 与评分占位不变；
/// 内部完成 plan → merge → 定点采集 → check 全链路。空注册表时无采集无 check、
/// findings 为空且不报错（spec 5.4.3.2）。
pub fn scan(path: &std::path::Path, registry: &CheckerRegistry) -> Result<ScanReport> {
    let started_at = now_secs();
    let start = Instant::now();

    let repo = GitRepo::open(path)?;
    let snapshot = repo.snapshot()?;

    // plan 阶段：base 上下文收集全部检查器的切片请求。
    let config = ScanConfig::default();
    let mut ctx = ScanContext::base(&snapshot, &config);
    let plans: Vec<_> = registry.list().iter().map(|c| c.plan(&ctx)).collect();
    let merged = merge_plans(&plans);

    // 定点采集：仅落地本特征两切片，其余切片保持未就绪（scan_engine 泛化点）。
    for slice in merged.slices() {
        match slice {
            DataSlice::FileContents { pattern } if pattern == "all" => {
                let metas = repo.workdir_file_metas()?;
                ctx.insert_slice(slice.clone(), SliceData::FileContents(metas));
            }
            DataSlice::FullHistory => {
                let blobs = repo.history_blob_metas(DEFAULT_BATCH_SIZE)?;
                ctx.insert_slice(slice.clone(), SliceData::FullHistory(blobs));
            }
            _ => {}
        }
    }

    // check 阶段：逐检查器纯计算产出 findings。
    let findings: Vec<_> = registry
        .list()
        .iter()
        .flat_map(|c| c.check(&ctx))
        .collect();

    let report = ScanReport {
        repo_path: path.display().to_string(),
        findings,
        scores: Scores {
            structure: 100,
            history: 100,
            branches: 100,
            deps: 100,
            security: 100,
        },
        started_at,
        duration_ms: start.elapsed().as_millis() as u64,
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    Ok(report)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::scan;
    use crate::checker::CheckerRegistry;
    use crate::testutil::TestRepo;

    /// ⑮ 引擎窄链路：空注册表扫描不报错，findings 为空（spec 5.4.3.2 衔接复验）。
    #[test]
    fn 引擎窄链路_空注册表扫描不报错() {
        let mut t = TestRepo::init();
        t.commit(&[("a.txt", b"hi")], "init");
        let registry = CheckerRegistry::new();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }
}
