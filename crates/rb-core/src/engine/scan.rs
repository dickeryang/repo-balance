//! 扫描编排（D3 实现）。当前为占位：打开仓库、取快照、返回报告骨架。
use std::time::Instant;

use crate::checker::CheckerRegistry;
use crate::error::Result;
use crate::git::GitRepo;
use crate::model::report::{ScanReport, Scores};

/// 对 `path` 执行完整扫描。
pub fn scan(path: &std::path::Path, _registry: &CheckerRegistry) -> Result<ScanReport> {
    let started_at = now_secs();
    let start = Instant::now();

    let repo = GitRepo::open(path)?;
    let _snapshot = repo.snapshot()?;

    // D3 在此消费 _snapshot + registry，产出 findings 并计算评分。
    let report = ScanReport {
        repo_path: path.display().to_string(),
        findings: Vec::new(),
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
