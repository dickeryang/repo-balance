//! IPC 命令层：五条命令桥接前端与 rb-core。
//!
//! stub 阶段：start_scan 异步包装同步 engine::scan，一次性 emit scan-done；
//! cancel_scan 设置 AtomicBool 但 engine::scan 不消费（待 D3-2 落地后切换）。

use std::path::PathBuf;

use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::dto::{RepoSnapshotDto, ScanReportDto};
use crate::error::map_error;
use crate::state::ScanState;

/// 打开系统目录选择对话框，返回选中路径（用户取消返回 None）。
#[tauri::command]
pub async fn select_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let result = rx.await.map_err(|e| e.to_string())?;
    Ok(result
        .and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
}

/// 获取仓库快照信息（提交数/分支数/文件数等）。
#[tauri::command]
pub fn repo_info(path: String) -> Result<RepoSnapshotDto, String> {
    let path = PathBuf::from(path);
    let repo = rb_core::git::GitRepo::open(&path).map_err(map_error)?;
    let snap = repo.snapshot().map_err(map_error)?;
    Ok(RepoSnapshotDto::from(snap))
}

/// 启动扫描（stub：异步包装同步 scan，完成后一次性 emit scan-done）。
///
/// stub 语义：无中间 scan-progress 事件；
/// 待 D3-2 scan_engine 落地后切换为 ScanEngine::run + progress 回调实时 emit。
#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
    path: String,
) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    rb_core::git::GitRepo::open(&path_buf).map_err(map_error)?;

    state.reset();
    state.set_scanning(true);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut registry = rb_core::checker::CheckerRegistry::new();
        let _ = registry.register(Box::new(rb_core::checker::BigFilesChecker));

        match rb_core::engine::scan::scan(&path_buf, &registry) {
            Ok(report) => {
                let dto = ScanReportDto::from(report);
                let _ = app_handle.emit("scan-done", dto);
            }
            Err(e) => {
                let _ = app_handle.emit("scan-cancelled", map_error(e));
            }
        }
    });
    Ok(())
}

/// 取消扫描（stub：设置 AtomicBool，当前 engine::scan 不消费该标志）。
///
/// stub 语义：取消请求记录但不立即生效；
/// 待 D3-2 落地后扫描编排层检测标志并产出 PartialReport 经 scan-cancelled 事件推送。
#[tauri::command]
pub fn cancel_scan(state: State<'_, ScanState>) -> Result<(), String> {
    if !state.is_scanning() {
        return Err("当前没有进行中的扫描".to_owned());
    }
    state.set_cancelled();
    Ok(())
}

/// 导出报告（Rust 侧写盘，evidence 保持掩码）。
#[tauri::command]
pub async fn export_report(
    app: tauri::AppHandle,
    format: String,
    report: ScanReportDto,
) -> Result<Option<String>, String> {
    let (content, ext, filter_name) = match format.as_str() {
        "json" => (
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
            "json",
            "JSON",
        ),
        "md" => (report_to_markdown(&report), "md", "Markdown"),
        _ => return Err("不支持的导出格式，仅支持 md 或 json".to_owned()),
    };

    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter(filter_name, &[ext])
        .save_file(move |path| {
            let _ = tx.send(path);
        });

    let result = rx.await.map_err(|e| e.to_string())?;
    match result {
        Some(path) => {
            let file_path = path
                .as_path()
                .ok_or_else(|| "无效的保存路径".to_owned())?
                .to_path_buf();
            std::fs::write(&file_path, content).map_err(|e| e.to_string())?;
            Ok(Some(file_path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

/// 将扫描报告格式化为 Markdown 文本。
fn report_to_markdown(report: &ScanReportDto) -> String {
    let mut md = String::new();
    md.push_str("# 仓衡扫描报告\n\n");
    md.push_str(&format!("**仓库**: {}\n", report.repo_path));
    md.push_str(&format!("**引擎版本**: {}\n", report.engine_version));
    md.push_str(&format!("**耗时**: {}ms\n\n", report.duration_ms));

    md.push_str("## 五维评分\n\n");
    for point in &report.radar_points {
        let score = point
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "- **{}**: {}（{} 条问题）\n",
            point.name, score, point.finding_count
        ));
    }

    md.push_str("\n## 发现的问题\n\n");
    if report.findings.is_empty() {
        md.push_str("未发现问题，仓库健康 ✓\n");
    } else {
        for f in &report.findings {
            md.push_str(&format!("### [{:?}] {}\n\n", f.severity, f.title));
            md.push_str(&format!("- **类别**: {:?}\n", f.category));
            md.push_str(&format!("- **证据**: {}\n", f.evidence));
            md.push_str(&format!("- **建议**: {}\n\n", f.suggestion));
        }
    }
    md
}