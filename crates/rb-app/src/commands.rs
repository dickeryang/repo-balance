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
/// `stage` 传递原始阶段标识（snapshot/workdir/history/check/done），
/// 由前端按当前 locale 翻译；字段与前端 `ProgressEvent` 类型
/// 一一对应（camelCase：stage/checkerId/done/total/percent）。
fn progress_payload(p: rb_core::engine::scan::ScanProgress) -> serde_json::Value {
    let percent = estimate_percent(p.stage, p.done, p.total);
    serde_json::json!({
        "stage": p.stage,
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
/// `config` 为 `None` 时使用默认配置（大文件阈值 1 MiB、启用 big-files）。
#[tauri::command]
pub async fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
    path: String,
    config: Option<crate::dto::ScanConfigDto>,
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
    let cfg = config.unwrap_or_default();

    tauri::async_runtime::spawn(async move {
        let app_for_blocking = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            // 进度事件由引擎阶段回调统一驱动（快照 → 工作区采集 →
            // 历史采集批次级细粒度进度 → 检查器逐个完成 → 完成）。
            let app_for_progress = app_for_blocking.clone();
            let progress_cb = move |p: rb_core::engine::scan::ScanProgress| {
                let _ = app_for_progress.emit("scan-progress", progress_payload(p));
            };
            // 按启用列表注册检查器（空列表表示全部启用）。
            let mut registry = rb_core::checker::CheckerRegistry::new();
            let enable_all = cfg.enabled_checkers.is_empty();
            if enable_all || cfg.enabled_checkers.iter().any(|id| id == "big-files") {
                let _ = registry.register(Box::new(rb_core::checker::BigFilesChecker));
            }
            let core_config = rb_core::checker::ScanConfig {
                big_file_threshold: cfg.big_file_threshold,
            };
            rb_core::engine::scan::scan_with_progress(
                &path_buf,
                &registry,
                Some(cancel_flag_arc.as_ref()),
                Some(&progress_cb as &dyn Fn(rb_core::engine::scan::ScanProgress)),
                &core_config,
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
        "html" => (report_to_html(&report), "html", "HTML"),
        _ => return Err("不支持的导出格式，仅支持 md、json 或 html".to_owned()),
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

/// 严重程度对应的主题色（与前端 --critical/--warning/--info 一致）。
fn severity_color(severity: &rb_core::model::finding::Severity) -> &'static str {
    use rb_core::model::finding::Severity;
    match severity {
        Severity::Critical => "#ff5c6c",
        Severity::Warning => "#ffb44d",
        Severity::Info => "#58c4a3",
    }
}

/// HTML 文本转义：& < > " → 实体，避免证据/建议破坏 HTML 结构。
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Unix 时间戳（秒）→ "YYYY-MM-DD HH:MM:SS UTC"（Howard Hinnant civil_from_days）。
fn format_time_utc(epoch: i64) -> String {
    if epoch < 0 {
        return format!("Unix 时间戳: {epoch}");
    }
    let secs = epoch as u64;
    let days = (secs / 86400) as i64;
    let sod = secs % 86400;
    let hour = sod / 3600;
    let min = (sod % 3600) / 60;
    let sec = sod % 60;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, m, d, hour, min, sec
    )
}

/// 生成内联 SVG 雷达图（复刻前端 RadarChart.vue 几何，含网格/轴线/评分多边形/标签）。
fn radar_svg(points: &[crate::dto::RadarPointDto]) -> String {
    const CX: f64 = 150.0;
    const CY: f64 = 150.0;
    const R: f64 = 105.0;
    const PI: f64 = std::f64::consts::PI;
    let angle = |i: usize| -> f64 { -PI / 2.0 + (i as f64) * 2.0 * PI / 5.0 };
    let vertex = |i: usize, r: f64| -> (f64, f64) {
        let a = angle(i);
        (CX + r * a.cos(), CY + r * a.sin())
    };

    let mut svg = String::from(
        "<svg width=\"300\" height=\"300\" viewBox=\"0 0 300 300\" xmlns=\"http://www.w3.org/2000/svg\">",
    );

    for &ratio in &[0.25_f64, 0.5, 0.75, 1.0] {
        let pts: Vec<String> = (0..5)
            .map(|i| {
                let (x, y) = vertex(i, R * ratio);
                format!("{:.1},{:.1}", x, y)
            })
            .collect();
        svg.push_str(&format!(
            "<polygon points=\"{}\" fill=\"none\" stroke=\"#2a3550\" stroke-width=\"1\"/>",
            pts.join(" ")
        ));
    }
    for i in 0..5 {
        let (x, y) = vertex(i, R);
        svg.push_str(&format!(
            "<line x1=\"{CX:.1}\" y1=\"{CY:.1}\" x2=\"{x:.1}\" y2=\"{y:.1}\" stroke=\"#2a3550\" stroke-width=\"1\"/>"
        ));
    }

    if !points.is_empty() {
        let score_pts: Vec<String> = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let r = (p.score.unwrap_or(0) as f64 / 100.0) * R;
                let (x, y) = vertex(i, r);
                format!("{:.1},{:.1}", x, y)
            })
            .collect();
        svg.push_str(&format!(
            "<polygon points=\"{}\" fill=\"rgba(79,140,255,0.25)\" stroke=\"#4f8cff\" stroke-width=\"2\"/>",
            score_pts.join(" ")
        ));
        for (i, p) in points.iter().enumerate() {
            let r = (p.score.unwrap_or(0) as f64 / 100.0) * R;
            let (x, y) = vertex(i, r);
            svg.push_str(&format!(
                "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"3\" fill=\"#4f8cff\"/>"
            ));
            let (lx, ly) = vertex(i, R + 18.0);
            let score_text = p
                .score
                .map(|s| s.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            svg.push_str(&format!(
                "<text x=\"{lx:.1}\" y=\"{ly:.1}\" text-anchor=\"middle\" dominant-baseline=\"middle\" fill=\"#8b95ab\" font-size=\"11\">{} {}</text>",
                html_escape(&p.name),
                score_text
            ));
        }
    }
    svg.push_str("</svg>");
    svg
}

/// 将扫描报告格式化为自包含 HTML（暗色主题 + 内联 SVG 雷达图快照）。
///
/// 无外部依赖、无外链资源，可独立分享；evidence 经 HTML 转义保持掩码。
fn report_to_html(report: &ScanReportDto) -> String {
    let total_score: u8 = {
        let scores: Vec<u8> = report.radar_points.iter().filter_map(|p| p.score).collect();
        if scores.is_empty() {
            0
        } else {
            (scores.iter().map(|s| *s as u32).sum::<u32>() / scores.len() as u32) as u8
        }
    };
    let total_class = if total_score >= 80 {
        "good"
    } else if total_score >= 60 {
        "mid"
    } else {
        "bad"
    };

    let mut dim_bars = String::new();
    for p in &report.radar_points {
        let pct = p.score.unwrap_or(0);
        let bar_color = if pct >= 80 {
            "#4fd07a"
        } else if pct >= 60 {
            "#ffb44d"
        } else {
            "#ff5c6c"
        };
        let score_text = p
            .score
            .map(|s| s.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        dim_bars.push_str(&format!(
            "<div class=\"dim-row\"><span>{}</span><div class=\"bar\"><i style=\"width:{}%;background:{}\"></i></div><span class=\"sc\">{}</span><span class=\"fc\">{} 条</span></div>",
            html_escape(&p.name),
            pct,
            bar_color,
            score_text,
            p.finding_count
        ));
    }

    let mut findings_html = String::new();
    if report.findings.is_empty() {
        findings_html.push_str("<p class=\"healthy\">未发现问题，仓库健康 ✓</p>");
    } else {
        for f in &report.findings {
            findings_html.push_str(&format!(
                "<div class=\"finding\"><div class=\"finding-head\"><span class=\"sev-tag\" style=\"color:{};background:rgba(0,0,0,0.2)\">[{}]</span><span class=\"cat-tag\">{}</span><span class=\"finding-title\">{}</span></div><div class=\"finding-body\"><div class=\"detail-box\"><div class=\"lbl\">证据（🔒 掩码展示）</div><div class=\"evidence\">{}</div></div><div class=\"detail-box\"><div class=\"lbl\">修复建议</div><div class=\"suggestion\">{}</div></div></div></div>",
                severity_color(&f.severity),
                severity_cn(&f.severity),
                category_cn(&f.category),
                html_escape(&f.title),
                html_escape(&f.evidence),
                html_escape(&f.suggestion),
            ));
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>仓衡扫描报告 · {}</title>
<style>
* {{ margin:0; padding:0; box-sizing:border-box; }}
body {{ background:#0f1420; color:#e6ebf5; font-family:-apple-system,"PingFang SC","Microsoft YaHei",sans-serif; padding:32px; }}
.container {{ max-width:960px; margin:0 auto; }}
h1 {{ font-size:24px; margin-bottom:8px; }}
h2 {{ font-size:18px; margin:24px 0 12px; }}
.meta {{ color:#8b95ab; font-size:13px; margin-bottom:20px; }}
.report-head {{ display:flex; gap:20px; margin-bottom:24px; }}
.score-card {{ background:#171e2e; border:1px solid #2a3550; border-radius:12px; padding:22px; width:240px; text-align:center; flex-shrink:0; }}
.score-big {{ font-size:64px; font-weight:800; line-height:1.1; }}
.score-big.good {{ color:#4fd07a; }} .score-big.mid {{ color:#ffb44d; }} .score-big.bad {{ color:#ff5c6c; }}
.score-label {{ color:#8b95ab; font-size:12px; margin-top:4px; }}
.radar-panel {{ background:#171e2e; border:1px solid #2a3550; border-radius:12px; padding:22px; flex:1; display:flex; gap:16px; align-items:center; }}
.radar-wrap {{ width:300px; height:300px; flex-shrink:0; }}
.dim-legend {{ flex:1; display:flex; flex-direction:column; gap:8px; }}
.dim-row {{ display:flex; align-items:center; gap:10px; background:#1c2438; border:1px solid #2a3550; border-radius:8px; padding:9px 14px; font-size:13px; }}
.bar {{ flex:1; height:6px; border-radius:999px; background:#2a3550; overflow:hidden; }}
.bar i {{ display:block; height:100%; border-radius:999px; }}
.sc {{ width:34px; text-align:right; font-weight:700; }}
.fc {{ font-size:11px; color:#8b95ab; width:52px; text-align:right; }}
.card {{ background:#171e2e; border:1px solid #2a3550; border-radius:12px; padding:22px; }}
.finding {{ background:#171e2e; border:1px solid #2a3550; border-radius:10px; margin-bottom:10px; overflow:hidden; }}
.finding-head {{ display:flex; align-items:center; gap:12px; padding:13px 16px; }}
.sev-tag {{ font-size:11px; font-weight:700; padding:2px 8px; border-radius:5px; }}
.cat-tag {{ font-size:11px; color:#8b95ab; border:1px solid #2a3550; padding:2px 8px; border-radius:5px; }}
.finding-title {{ font-weight:600; font-size:13px; flex:1; }}
.finding-body {{ padding:4px 16px 16px; border-top:1px solid #2a3550; display:grid; grid-template-columns:1fr 1fr; gap:14px; margin-top:12px; }}
.detail-box {{ background:#1c2438; border-radius:8px; padding:12px 14px; }}
.lbl {{ font-size:11px; color:#8b95ab; margin-bottom:6px; }}
.evidence {{ font-family:"SF Mono",Menlo,monospace; font-size:12px; color:#ffd9dd; word-break:break-all; line-height:1.7; white-space:pre-wrap; }}
.suggestion {{ font-size:13px; line-height:1.7; border-left:3px solid #4f8cff; background:rgba(79,140,255,0.15); padding:10px 14px; border-radius:0 8px 8px 0; }}
.healthy {{ color:#4fd07a; text-align:center; padding:16px; }}
footer {{ color:#8b95ab; font-size:12px; text-align:center; margin-top:32px; }}
</style>
</head>
<body>
<div class="container">
<h1>仓衡 · Git 仓库健康度体检报告</h1>
<div class="meta">仓库：{} · 引擎版本 {} · 扫描时间 {} · 耗时 {}ms{}</div>
<div class="report-head">
<div class="score-card">
<div class="score-big {}">{}</div>
<div class="score-label">五维平均分（0~100）</div>
</div>
<div class="radar-panel">
<div class="radar-wrap">{}</div>
<div class="dim-legend">{}</div>
</div>
</div>
<h2>发现的问题（{} 条）</h2>
<div class="card">{}</div>
<footer>由仓衡 RepoBalance 生成 · evidence 已掩码 · 报告自包含可离线分享</footer>
</div>
</body>
</html>"#,
        html_escape(&report.repo_path),
        html_escape(&report.repo_path),
        html_escape(&report.engine_version),
        format_time_utc(report.started_at),
        report.duration_ms,
        if report.cancelled { " · 扫描已取消（显示已完成部分）" } else { "" },
        total_class,
        total_score,
        radar_svg(&report.radar_points),
        dim_bars,
        report.findings.len(),
        findings_html,
    )
}
