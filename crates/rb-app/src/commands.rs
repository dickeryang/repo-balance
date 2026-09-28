//! IPC 命令层：五条命令桥接前端与 rb-core。
//!
//! start_scan 异步包装同步 engine::scan，完成后 emit scan-done；
//! cancel_scan 设置 AtomicBool，引擎在批次边界消费该标志并提前返回，
//! 扫描结束后 emit scan-cancelled 通知前端。

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, Manager, State};
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
///
/// 异步命令：快照需全量遍历提交与文件树，大仓库耗时明显，
/// 放入 spawn_blocking 执行，避免同步命令阻塞主线程冻结 UI。
#[tauri::command]
pub async fn repo_info(path: String) -> Result<RepoSnapshotDto, String> {
    let path_buf = PathBuf::from(&path);
    let snap = tauri::async_runtime::spawn_blocking(move || {
        let repo = rb_core::git::GitRepo::open(&path_buf).map_err(map_error)?;
        repo.snapshot().map_err(map_error)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(RepoSnapshotDto::from(snap))
}

/// 将引擎阶段事件映射为前端进度事件 payload。
///
/// 中文阶段名 + 估算百分比（0-100），字段与前端 `ProgressEvent` 类型
/// 一一对应（camelCase：stage/checkerId/done/total/percent）。
fn progress_payload(p: rb_core::engine::scan::ScanProgress) -> serde_json::Value {
    let stage_cn = match p.stage {
        "snapshot" => "初始化",
        "workdir" => "工作区采集",
        "history" => "历史采集",
        "check" => "检查",
        _ => "完成",
    };
    let percent = estimate_percent(p.stage, p.done, p.total);
    serde_json::json!({
        "stage": stage_cn,
        "checkerId": "",
        "done": p.done,
        "total": p.total,
        "percent": percent,
    })
}

/// 按阶段将 (done, total) 映射为 0-100 的估算百分比。
///
/// 分段：初始化 0-5 → 工作区采集 5-10 → 历史采集 10-90（按提交比例线性）→
/// 检查 90-99 → 完成 100。
fn estimate_percent(stage: &str, done: usize, total: usize) -> u8 {
    match stage {
        "snapshot" => 5,
        "workdir" => 10,
        "history" => {
            if total == 0 {
                10
            } else {
                let ratio = (done as f64 / total as f64).min(1.0);
                (10.0 + 80.0 * ratio).round() as u8
            }
        }
        "check" => {
            if total == 0 {
                90
            } else {
                (90.0 + 9.0 * (done as f64 / total as f64)).round() as u8
            }
        }
        _ => 100,
    }
}

/// 启动扫描：异步包装同步 scan（spawn_blocking 避免占死 tokio worker），
/// 完成后 emit scan-done；失败 emit scan-error（与用户取消事件分离）。
#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
    path: String,
) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    rb_core::git::GitRepo::open(&path_buf).map_err(map_error)?;

    // CAS 领取扫描权：复位取消标志并原子置位 scanning，拒绝并发扫描。
    if !state.begin_scan() {
        return Err("已有扫描正在进行，请先等待完成或取消".to_owned());
    }

    let app_handle = app.clone();
    let cancel_flag = state.cancel_flag();
    let cancel_flag_arc = Arc::clone(&cancel_flag);

    tauri::async_runtime::spawn(async move {
        let app_for_blocking = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            // 进度事件由引擎阶段回调统一驱动（快照 → 工作区采集 →
            // 历史采集批次级细粒度进度 → 检查器逐个完成 → 完成）。
            let app_for_progress = app_for_blocking.clone();
            let progress_cb = move |p: rb_core::engine::scan::ScanProgress| {
                let _ = app_for_progress.emit("scan-progress", progress_payload(p));
            };
            let mut registry = rb_core::checker::CheckerRegistry::new();
            let _ = registry.register(Box::new(rb_core::checker::BigFilesChecker));
            rb_core::engine::scan::scan_with_progress(
                &path_buf,
                &registry,
                Some(cancel_flag_arc.as_ref()),
                Some(&progress_cb as &dyn Fn(rb_core::engine::scan::ScanProgress)),
            )
        })
        .await;

        let state = app_handle.state::<ScanState>();
        let was_cancelled = state.is_cancelled();
        state.set_scanning(false);

        match result {
            Ok(Ok(report)) => {
                let dto = ScanReportDto::from(report);
                if was_cancelled {
                    let _ = app_handle.emit("scan-cancelled", "扫描已取消".to_owned());
                } else {
                    let _ = app_handle.emit("scan-done", dto);
                }
            }
            Ok(Err(e)) => {
                let _ = app_handle.emit("scan-error", map_error(e));
            }
            Err(e) => {
                let _ = app_handle.emit("scan-error", e.to_string());
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
            md.push_str(&format!("### [{}] {}\n\n", severity_cn(&f.severity), f.title));
            md.push_str(&format!("- **类别**: {}\n", category_cn(&f.category)));
            md.push_str(&format!("- **证据**: {}\n", f.evidence));
            md.push_str(&format!("- **建议**: {}\n\n", f.suggestion));
        }
    }
    md
}

/// 严重程度的中文文案（与前端展示口径一致）。
fn severity_cn(severity: &rb_core::model::finding::Severity) -> &'static str {
    use rb_core::model::finding::Severity;
    match severity {
        Severity::Critical => "严重",
        Severity::Warning => "警告",
        Severity::Info => "提示",
    }
}

/// 检查维度的中文文案（与前端展示口径一致）。
fn category_cn(category: &rb_core::model::finding::Category) -> &'static str {
    use rb_core::model::finding::Category;
    match category {
        Category::Structure => "结构",
        Category::History => "历史",
        Category::Branches => "分支",
        Category::Deps => "依赖",
        Category::Security => "安全",
    }
}
