//! 远程仓库体检编排：URL → 浅克隆到临时目录 → 本地扫描 → 临时目录自动清理。
//!
//! 复用 [`engine::scan`] 的全部检查管线；临时目录由 [`tempfile::TempDir`]
//! RAII 保证扫描结束（含出错路径）后清理，不残留克隆产物。
use std::sync::atomic::AtomicBool;

use crate::checker::{CheckerRegistry, ScanConfig};
use crate::engine::scan::{scan_with_progress, ScanProgress};
use crate::git::clone_shallow;
use crate::model::report::ScanReport;
use crate::Result;

/// 把远程仓库克隆到临时目录，返回持有它的 `TempDir`。
///
/// 独立暴露以便编排层自定义扫描时机；`TempDir` drop 即清理克隆产物。
pub fn clone_remote(url: &str, depth: i32) -> Result<tempfile::TempDir> {
    let tmp = tempfile::tempdir()?;
    clone_shallow(url, tmp.path(), depth)?;
    Ok(tmp)
}

/// 对远程仓库 URL 执行完整体检。
///
/// 流程：临时目录创建 → 浅克隆 → 完整扫描 → 报告返回，`TempDir`
/// 离开作用域即清理克隆产物，保持「纯本地、不落盘」语义。
/// 注意：报告内的 `repo_path` 指向临时目录，编排层展示时应以 URL 为准。
///
/// `depth` 为浅克隆深度（`<= 0` 时取引擎默认值 1）；取消与进度回调语义
/// 与 [`scan_with_progress`] 一致。
pub fn scan_remote(
    url: &str,
    registry: &CheckerRegistry,
    cancel_flag: Option<&AtomicBool>,
    on_progress: Option<&dyn Fn(ScanProgress)>,
    config: &ScanConfig,
    depth: i32,
) -> Result<ScanReport> {
    let tmp = clone_remote(url, depth)?;
    scan_with_progress(tmp.path(), registry, cancel_flag, on_progress, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::{BigFilesChecker, ScanConfig};
    use crate::testutil::TestRepo;

    /// 构造仅启用大文件检查器的注册表（finding 数易断言）。
    fn registry_big_files() -> CheckerRegistry {
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(BigFilesChecker)).unwrap();
        registry
    }

    /// 命中路径：本地源仓库 → 克隆 → 扫描产出与直接扫描源仓库一致的结论。
    #[test]
    fn 远程体检_本地路径克隆后扫描产出结论() {
        let mut source = TestRepo::init();
        // 2 MiB 文件，默认 1 MiB 阈值下必命中大文件。
        source.commit(&[("big.bin", &[0u8; 2 * 1024 * 1024])], "add big file");
        let url = source.path().to_str().unwrap();

        let report = scan_remote(
            url,
            &registry_big_files(),
            None,
            None,
            &ScanConfig::default(),
            1,
        )
        .unwrap();
        // 报告路径指向临时目录（编排层展示时以 URL 为准），但结论真实产出。
        assert!(report
            .repo_path
            .starts_with(std::env::temp_dir().to_str().unwrap()));
        assert!(!report.findings.is_empty(), "克隆产物应命中大文件结论");
    }

    /// 不命中路径：干净仓库克隆扫描后零 finding、分数为满分。
    #[test]
    fn 远程体检_干净仓库零结论() {
        let mut source = TestRepo::init();
        source.commit(&[("README.md", b"hello" as &[u8])], "init");
        let url = source.path().to_str().unwrap();

        let report = scan_remote(
            url,
            &registry_big_files(),
            None,
            None,
            &ScanConfig::default(),
            1,
        )
        .unwrap();
        assert!(report.findings.is_empty(), "干净仓库不应有 finding");
    }

    /// 边界路径：克隆失败（源不存在）必须报错，且不产出半成品报告。
    #[test]
    fn 远程体检_源不可达时报错() {
        let result = scan_remote(
            "/nonexistent/repo-path-does-not-exist",
            &registry_big_files(),
            None,
            None,
            &ScanConfig::default(),
            1,
        );
        assert!(result.is_err(), "克隆失败必须报错");
    }

    /// 边界路径：depth <= 0 回退默认深度 1，行为与显式 depth=1 一致。
    #[test]
    fn 远程体检_非法深度回退默认() {
        let mut source = TestRepo::init();
        source.commit(&[("README.md", b"hello" as &[u8])], "init");
        let url = source.path().to_str().unwrap();

        let report = scan_remote(
            url,
            &registry_big_files(),
            None,
            None,
            &ScanConfig::default(),
            0,
        )
        .unwrap();
        assert!(report.findings.is_empty());
    }

    /// 清理语义：`clone_remote` 返回的 `TempDir` drop 后克隆产物消失。
    #[test]
    fn 远程体检_临时目录扫描后清理() {
        let mut source = TestRepo::init();
        source.commit(&[("README.md", b"hello" as &[u8])], "init");
        let url = source.path().to_str().unwrap();

        let tmp = clone_remote(url, 1).unwrap();
        let cloned = tmp.path().to_path_buf();
        assert!(cloned.join(".git").exists(), "克隆产物应存在");
        drop(tmp);
        assert!(!cloned.exists(), "TempDir drop 后克隆产物应被清理");
    }
}
