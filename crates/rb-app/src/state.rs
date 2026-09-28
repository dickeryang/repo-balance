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

    /// 原子领取扫描权：复位取消标志，并以 CAS 确认 `scanning` false→true。
    ///
    /// 返回 `false` 表示已有扫描在进行（原「检查-复位-置位」三步非原子，
    /// 并发 start_scan 可能同时通过检查；本方法以 compare_exchange 消除竞态）。
    pub fn begin_scan(&self) -> bool {
        self.cancel_flag.store(false, Ordering::Relaxed);
        self.scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
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