//! 扫描状态管理：取消标志与扫描中标记。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 扫描运行时状态：持有取消标志与扫描中标记。
///
/// `Send + Sync` 安全，经 `tauri::Builder::manage` 注入。
pub struct ScanState {
    cancel_flag: Arc<AtomicBool>,
    scanning: AtomicBool,
}

impl ScanState {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            scanning: AtomicBool::new(false),
        }
    }

    /// 返回取消标志的 Arc 引用，供引擎在批次边界消费。
    pub fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel_flag
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn set_cancelled(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// 重置取消标志与扫描标记（`start_scan` 前调用，防止上一轮残留）。
    pub fn reset(&self) {
        self.cancel_flag.store(false, Ordering::Relaxed);
        self.scanning.store(false, Ordering::Relaxed);
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::Relaxed)
    }

    pub fn set_scanning(&self, val: bool) {
        self.scanning.store(val, Ordering::Relaxed);
    }
}

impl Default for ScanState {
    fn default() -> Self {
        Self::new()
    }
}