//! IPC 命令层：五条命令桥接前端与 rb-core。
//!
//! start_scan 异步包装同步 engine::scan，完成后 emit scan-done
//! （取消的扫描推送 cancelled=true 的部分报告，同样经 scan-done）；
//! 失败 emit scan-error。

use std::path::PathBuf;

use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::dto::{CustomRuleDto, RepoSnapshotDto, ScaleEstimateDto, ScanReportDto};
use crate::error::map_error;
use crate::state::ScanState;
use rb_core::model::vuln::VulnIndex;
use rb_core::rules::{self, CustomRule};

/// 扫描历史文件名（存放于应用数据目录）。
const HISTORY_FILE_NAME: &str = "scan-history.json";

/// 离线漏洞库索引文件名（存放于应用数据目录；缺失即不做漏洞匹配）。
const VULN_INDEX_FILE_NAME: &str = "vuln-index.jsonl";

/// 当前 Unix 秒（系统调用失败时回退 0——仅用于时间窗换算）。
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// 打开系统目录选择对话框，返回选中路径（用户取消返回 None）。
#[tauri::command]
pub async fn select_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let result = rx.await.map_err(|e| e.to_string())?;
    Ok(result.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string())))
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
/// 由前端按当前 locale 翻译；`check` 阶段携带 `checkerId`（进入时）与
/// `findingCount`（完成时）；字段与前端 `ProgressEvent` 类型
/// 一一对应（camelCase：stage/checkerId/done/total/percent）。
fn progress_payload(p: rb_core::engine::scan::ScanProgress) -> serde_json::Value {
    let percent = estimate_percent(p.stage, p.done, p.total);
    serde_json::json!({
        "stage": p.stage,
        "checkerId": p.checker_id,
        "done": p.done,
        "total": p.total,
        "percent": percent,
        "findingCount": p.finding_count,
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

/// 扫描来源：本地路径，或远程 URL（浅克隆到临时目录后扫描）。
pub enum ScanSource {
    /// 本地磁盘上的仓库路径。
    Local(PathBuf),
    /// 远程仓库 URL 与浅克隆深度。
    Remote { url: String, depth: i32 },
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
    launch_scan(app, state, ScanSource::Local(path_buf), config).await
}

/// 远程仓库体检：URL 浅克隆到临时目录 → 完整扫描 → 临时目录自动清理。
///
/// 报告内的 `repo_path` 以 URL 为准——临时目录是实现细节，不应出现在
/// 报告页与扫描历史中。
#[tauri::command]
pub async fn start_remote_scan(
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
    url: String,
    depth: Option<i32>,
    config: Option<crate::dto::ScanConfigDto>,
) -> Result<(), String> {
    let url = url.trim().to_owned();
    if url.is_empty() {
        return Err("仓库 URL 不能为空".to_owned());
    }
    launch_scan(
        app,
        state,
        ScanSource::Remote {
            url,
            depth: depth.unwrap_or(1),
        },
        config,
    )
    .await
}

/// 本地与远程扫描共用的启动流程：规则校验 → CAS 领取扫描权 →
/// spawn_blocking 内解析来源（远程先浅克隆）→ 引擎扫描 → 事件推送。
async fn launch_scan(
    app: tauri::AppHandle,
    state: State<'_, ScanState>,
    source: ScanSource,
    config: Option<crate::dto::ScanConfigDto>,
) -> Result<(), String> {
    let cfg = config.unwrap_or_default();
    // 自定义规则：先整体校验编译再启动扫描。GUI 侧已做同样校验，这里是兜底；
    // 规则是用户显式配置，非法就快速失败，绝不静默降级成「规则已生效」的假象。
    let compiled_rules: Vec<rules::CompiledRule> = if cfg.custom_rules.is_empty() {
        Vec::new()
    } else {
        let rules: Vec<CustomRule> = cfg.custom_rules.iter().cloned().map(Into::into).collect();
        rules::compile_rules(&rules).map_err(|e| e.to_string())?
    };

    // CAS 领取扫描权：复位取消标志并原子置位 scanning，拒绝并发扫描。
    if !state.begin_scan() {
        return Err("已有扫描正在进行，请先等待完成或取消".to_owned());
    }

    let app_handle = app.clone();
    let cancel_flag = state.cancel_flag();
    let cancel_flag_arc = cancel_flag.clone();
    // 受限扫描的范围说明随报告一起推送，前端与导出据此如实标注。
    let scope_note = scope_note_of(&cfg);

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
            let enabled = |id: &str| enable_all || cfg.enabled_checkers.iter().any(|e| e == id);
            if enabled("big-files") {
                let _ = registry.register(Box::new(rb_core::checker::BigFilesChecker));
            }
            if enabled("secrets") {
                let _ = registry.register(Box::new(rb_core::checker::SecretsChecker));
            }
            if enabled("conflict-markers") {
                let _ = registry.register(Box::new(rb_core::checker::ConflictMarkersChecker));
            }
            if enabled("zombie-branches") {
                let _ = registry.register(Box::new(rb_core::checker::ZombieBranchesChecker));
            }
            if enabled("commit-habits") {
                let _ = registry.register(Box::new(rb_core::checker::CommitHabitsChecker));
            }
            if enabled("deps-health") {
                let _ = registry.register(Box::new(rb_core::checker::DepsHealthChecker));
            }
            // 自定义规则检查器由「是否有规则」驱动（不提供独立开关）：无规则时
            // 注册它会把没有任何规则的维度错误标记为已覆盖（假满分）。
            if !compiled_rules.is_empty() {
                let _ = registry.register(Box::new(
                    rb_core::checker::CustomRulesChecker::from_compiled(compiled_rules),
                ));
            }
            let mut core_config = cfg.to_core_config(now_secs());
            // 漏洞库在壳层载入（rb-core 不做文件 I/O）；缺失/损坏时静默降级为
            // 「不做漏洞匹配」，不因可选增强失败而中断扫描。
            core_config.vuln_index = load_vuln_index(&app);
            match &source {
                ScanSource::Local(path_buf) => rb_core::engine::scan::scan_with_progress(
                    path_buf,
                    &registry,
                    Some(cancel_flag_arc.as_ref()),
                    Some(&progress_cb as &dyn Fn(rb_core::engine::scan::ScanProgress)),
                    &core_config,
                ),
                ScanSource::Remote { url, depth } => {
                    // 远程来源：临时目录 RAII 保证扫描结束（含出错路径）后
                    // 清理克隆产物；报告内 repo_path 覆写为 URL——临时目录
                    // 是实现细节，不应出现在报告页与扫描历史里。
                    let tmp = tempfile::tempdir().map_err(rb_core::RbError::Io)?;
                    eprintln!("浅克隆 {url} (depth={depth}) …");
                    rb_core::git::clone_shallow(url, tmp.path(), *depth)?;
                    let mut report = rb_core::engine::scan::scan_with_progress(
                        tmp.path(),
                        &registry,
                        Some(cancel_flag_arc.as_ref()),
                        Some(&progress_cb as &dyn Fn(rb_core::engine::scan::ScanProgress)),
                        &core_config,
                    )?;
                    report.repo_path = url.clone();
                    Ok(report)
                }
            }
        })
        .await;

        let state = app_handle.state::<ScanState>();
        let was_cancelled = state.is_cancelled();
        state.set_scanning(false);

        match result {
            Ok(Ok(report)) => {
                // 取消时引擎返回的是已采集的部分报告：置 cancelled 标记后
                // 仍经 scan-done 推送，前端照常渲染「部分结果」报告页。
                let mut dto = ScanReportDto::from(report);
                if was_cancelled {
                    dto.cancelled = true;
                }
                dto.scope_note = scope_note;
                let _ = app_handle.emit("scan-done", dto);
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

/// 载入离线漏洞库索引（应用数据目录下 `vuln-index.jsonl`）。
///
/// 缺失 / 损坏 / 读取失败一律返回 `None`：漏洞检测是**可选增强**，
/// 不应让扫描失败——未接入时依赖维度照常做结构检查。
pub(crate) fn load_vuln_index(app: &tauri::AppHandle) -> Option<std::sync::Arc<VulnIndex>> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join(VULN_INDEX_FILE_NAME);
    let text = std::fs::read_to_string(&path).ok()?;
    match rb_core::vuln::load_index(&text) {
        Ok(index) => {
            let count = index.len();
            // 载入失败不阻断扫描，但要留下可诊断的痕迹。
            if count == 0 {
                eprintln!("警告: 漏洞库 {VULN_INDEX_FILE_NAME} 为空，跳过漏洞匹配");
                return None;
            }
            eprintln!("已载入漏洞库: {count} 条公告");
            Some(std::sync::Arc::new(index))
        }
        Err(e) => {
            eprintln!("警告: 漏洞库解析失败，跳过漏洞匹配: {e}");
            None
        }
    }
}

/// 历史范围受限时的告知文案；未限流（两项均空/0）返回 `None`。
///
/// 受限结论必须与全量结论区分，否则「没扫到的老历史」会被误读为「没有问题」。
fn scope_note_of(cfg: &crate::dto::ScanConfigDto) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(n) = cfg.max_commits.filter(|n| *n > 0) {
        parts.push(format!("最新 {n} 个提交"));
    }
    if let Some(d) = cfg.since_days.filter(|d| *d > 0) {
        parts.push(format!("最近 {d} 天"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!(
        "历史范围: {}（更老的提交未参与本次结论）",
        parts.join(" · ")
    ))
}

/// 扫描前的规模预估：提交数/文件数为精确值，历史对象数与耗时为采样外推的粗估。
///
/// 异步命令：与快照同量级的 I/O（revwalk + 目录遍历），放入 spawn_blocking。
#[tauri::command]
pub async fn scan_estimate(path: String) -> Result<ScaleEstimateDto, String> {
    let path_buf = PathBuf::from(&path);
    let est = tauri::async_runtime::spawn_blocking(move || {
        let repo = rb_core::git::GitRepo::open(&path_buf).map_err(map_error)?;
        repo.scale_estimate().map_err(map_error)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(ScaleEstimateDto::from(est))
}

/// 扫描历史文件路径（应用数据目录下 `scan-history.json`）。
///
/// 目录不存在时自动创建；解析失败返回用户可读的中文错误（不泄露内部细节）。
fn history_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "无法定位应用数据目录".to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建应用数据目录失败: {e}"))?;
    Ok(dir.join(HISTORY_FILE_NAME))
}

/// 读取扫描历史（JSON 数组字符串）；文件不存在时返回 `[]`。
///
/// 历史持久化从 localStorage 迁至应用数据目录：突破 5MB 上限、不随浏览器
/// 缓存清理丢失、可被用户直接备份。数据形状由前端定义，此处按不透明字符串搬运。
#[tauri::command]
pub async fn load_scan_history(app: tauri::AppHandle) -> Result<String, String> {
    let path = history_file(&app)?;
    match std::fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => Ok(content),
        Ok(_) => Ok("[]".to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("[]".to_owned()),
        Err(e) => Err(format!("读取扫描历史失败: {e}")),
    }
}

/// 写入扫描历史（覆盖写，先落临时文件再 rename，避免半截写入损坏历史）。
#[tauri::command]
pub async fn save_scan_history(app: tauri::AppHandle, data: String) -> Result<(), String> {
    let path = history_file(&app)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data).map_err(|e| format!("写入扫描历史失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("保存扫描历史失败: {e}"))
}

/// 导入扫描历史：文件对话框选 JSON → 校验为数组 → 返回内容交给前端合并。
///
/// 在此校验「顶层必须是 JSON 数组」，避免把任意文件塞进历史把既有数据弄坏。
#[tauri::command]
pub async fn import_scan_history(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });

    let result = rx.await.map_err(|e| e.to_string())?;
    let Some(path) = result else {
        return Ok(None); // 用户取消
    };
    let file_path = path
        .as_path()
        .ok_or_else(|| "无效的文件路径".to_owned())?
        .to_path_buf();

    let content = std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {e}"))?;
    validate_history_json(&content)?;
    Ok(Some(content))
}

/// 校验导入内容顶层为 JSON 数组；非数组或非法 JSON 均给出可读错误。
///
/// 单独抽函数便于单测——对话框本身依赖运行时，无法在测试中驱动。
fn validate_history_json(content: &str) -> Result<(), String> {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(v) if v.is_array() => Ok(()),
        Ok(_) => Err("导入失败：文件顶层不是 JSON 数组".to_owned()),
        Err(e) => Err(format!("导入失败：不是合法的 JSON（{e}）")),
    }
}

/// 导出扫描历史：保存对话框选路径 → 写盘 → 返回实际路径（用户取消时为 `None`）。
#[tauri::command]
pub async fn export_scan_history(
    app: tauri::AppHandle,
    data: String,
) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("repo-balance-history.json")
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
            std::fs::write(&file_path, data).map_err(|e| format!("导出失败: {e}"))?;
            Ok(Some(file_path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

/// 导入自定义规则：文件对话框选 JSON → 整体校验（结构 / 正则 / id 唯一）→ 返回规则列表。
///
/// 校验在 rb-core 侧统一完成（`load_rules_json`）：**要么全部生效、要么明确报错**，
/// 不存在「一半规则静默生效」的中间态。
#[tauri::command]
pub async fn import_custom_rules(
    app: tauri::AppHandle,
) -> Result<Option<Vec<CustomRuleDto>>, String> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });

    let result = rx.await.map_err(|e| e.to_string())?;
    let Some(path) = result else {
        return Ok(None); // 用户取消
    };
    let file_path = path
        .as_path()
        .ok_or_else(|| "无效的文件路径".to_owned())?
        .to_path_buf();

    let content = std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {e}"))?;
    let rules = rules::load_rules_json(&content).map_err(|e| e.to_string())?;
    Ok(Some(rules.into_iter().map(CustomRuleDto::from).collect()))
}

/// 导出自定义规则：先校验（不导出不可用的规则集）→ 保存对话框 → 写盘 → 返回实际路径。
#[tauri::command]
pub async fn export_custom_rules(
    app: tauri::AppHandle,
    rules: Vec<CustomRuleDto>,
) -> Result<Option<String>, String> {
    let content = custom_rules_json(&rules)?;

    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("repo-balance-rules.json")
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
            std::fs::write(&file_path, content).map_err(|e| format!("导出失败: {e}"))?;
            Ok(Some(file_path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

/// 校验并序列化规则集为 JSON 文本（导出前统一走这里，保证导出内容可用）。
///
/// 单独抽函数便于单测——对话框本身依赖运行时，无法在测试中驱动。
fn custom_rules_json(rules: &[CustomRuleDto]) -> Result<String, String> {
    let core: Vec<CustomRule> = rules.iter().cloned().map(Into::into).collect();
    rules::compile_rules(&core).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(rules).map_err(|e| e.to_string())
}

/// 取消扫描（设置 AtomicBool 取消标志）。
///
/// engine::scan 编排层轮询该标志，检测到后中止扫描，
/// 已完成部分作为部分报告经 scan-done 事件推送（cancelled=true）。
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
    // 受限扫描必须如实在报告里标注，避免受限结论被当成全量结论。
    if let Some(scope) = &report.scope_note {
        md.push_str(&format!("**⚠ {scope}**\n\n"));
    }

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
            md.push_str(&format!(
                "### [{}] {}\n\n",
                severity_cn(&f.severity),
                f.title
            ));
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
.scope-note {{ background:rgba(255,176,32,0.12); border:1px solid rgba(255,176,32,0.4); color:#ffd08a; border-radius:8px; padding:10px 14px; font-size:12px; margin-bottom:20px; }}
footer {{ color:#8b95ab; font-size:12px; text-align:center; margin-top:32px; }}
</style>
</head>
<body>
<div class="container">
<h1>仓衡 · Git 仓库健康度体检报告</h1>
<div class="meta">仓库：{} · 引擎版本 {} · 扫描时间 {} · 耗时 {}ms{}</div>
{}
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
        if report.cancelled {
            " · 扫描已取消（显示已完成部分）"
        } else {
            ""
        },
        // 历史范围受限提示（全量扫描为空串，不占位）。
        report
            .scope_note
            .as_deref()
            .map(|s| format!("<div class=\"scope-note\">⚠ {}</div>", html_escape(s)))
            .unwrap_or_default(),
        total_class,
        total_score,
        radar_svg(&report.radar_points),
        dim_bars,
        report.findings.len(),
        findings_html,
    )
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{custom_rules_json, scope_note_of, validate_history_json};
    use crate::dto::{CustomRuleDto, ScanConfigDto};
    use rb_core::model::finding::{Category, Severity};

    /// 构造一条最小可用规则的 DTO。
    fn rule_dto(id: &str) -> CustomRuleDto {
        CustomRuleDto {
            id: id.into(),
            title: "禁用 console.log".into(),
            severity: Severity::Warning,
            category: Category::Structure,
            pattern: r"console\.log".into(),
            path_glob: None,
            ignore_case: false,
            suggestion: "改用统一日志封装".into(),
        }
    }

    /// 导出前校验：合法规则集序列化为顶层数组 JSON。
    #[test]
    fn 规则导出_合法规则集序列化为数组() {
        let json = custom_rules_json(&[rule_dto("no-console")]).unwrap();
        assert!(json.trim_start().starts_with('['), "{json}");
        assert!(json.contains("\"pathGlob\""), "{json}");
        assert!(json.contains("no-console"), "{json}");
    }

    /// 导出前校验：非法正则 / 重复 id 的规则集被拒绝（不产出不可用文件）。
    #[test]
    fn 规则导出_非法规则集被拒绝() {
        let mut bad_regex = rule_dto("bad");
        bad_regex.pattern = "([".into();
        let err = custom_rules_json(&[bad_regex]).unwrap_err();
        assert!(err.contains("正则无法编译"), "{err}");

        let err = custom_rules_json(&[rule_dto("dup"), rule_dto("dup")]).unwrap_err();
        assert!(err.contains("dup"), "{err}");
    }

    /// 空规则集可导出为空数组（清空规则的备份语义）。
    #[test]
    fn 规则导出_空规则集合法() {
        assert_eq!(custom_rules_json(&[]).unwrap(), "[]");
    }

    /// 未限流（全部为空）不产出范围说明。
    #[test]
    fn 范围说明_未限流时为None() {
        assert_eq!(scope_note_of(&ScanConfigDto::default()), None);
    }

    /// 0 值按「不限」处理，与 dto 换算口径一致。
    #[test]
    fn 范围说明_零值按不限处理() {
        let cfg = ScanConfigDto {
            max_commits: Some(0),
            since_days: Some(0),
            ..Default::default()
        };
        assert_eq!(scope_note_of(&cfg), None);
    }

    /// 单项限流：只标注该项。
    #[test]
    fn 范围说明_单项限流() {
        let cfg = ScanConfigDto {
            max_commits: Some(500),
            ..Default::default()
        };
        let note = scope_note_of(&cfg).unwrap();
        assert!(note.contains("最新 500 个提交"), "{note}");
        assert!(!note.contains("最近"), "{note}");
    }

    /// 双项限流：以「·」连接两项。
    #[test]
    fn 范围说明_双项限流() {
        let cfg = ScanConfigDto {
            max_commits: Some(500),
            since_days: Some(365),
            ..Default::default()
        };
        let note = scope_note_of(&cfg).unwrap();
        assert!(note.contains("最新 500 个提交"), "{note}");
        assert!(note.contains("最近 365 天"), "{note}");
        assert!(note.contains("·"), "{note}");
    }

    /// 导入校验：顶层数组（含空数组）通过。
    #[test]
    fn 导入校验_顶层数组通过() {
        assert!(validate_history_json("[]").is_ok());
        assert!(validate_history_json(r#"[{"id":"a"}]"#).is_ok());
    }

    /// 导入校验：非数组（对象 / 标量）与非法 JSON 均拒绝，避免脏数据覆盖历史。
    #[test]
    fn 导入校验_非数组与非法JSON被拒绝() {
        for bad in [
            r#"{"id":"a"}"#,
            "null",
            "\"str\"",
            "123",
            "",
            "[{]",
            "not json",
        ] {
            let err = validate_history_json(bad).unwrap_err();
            assert!(!err.is_empty(), "应给出错误: {bad}");
        }
    }

    /// Markdown 导出携带范围说明（受限结论不可被当成全量结论）。
    #[test]
    fn 导出_markdown携带范围说明() {
        let mut report = crate::dto::ScanReportDto::from(rb_core::model::report::ScanReport {
            repo_path: "/tmp/demo".into(),
            findings: Vec::new(),
            scores: rb_core::model::report::Scores {
                structure: Some(100),
                history: Some(100),
                branches: Some(100),
                deps: Some(100),
                security: Some(100),
            },
            started_at: 1,
            duration_ms: 2,
            checker_timings: Vec::new(),
            engine_version: "0.1.0".into(),
        });
        report.scope_note = scope_note_of(&ScanConfigDto {
            since_days: Some(30),
            ..Default::default()
        });
        let md = super::report_to_markdown(&report);
        assert!(md.contains("⚠ 历史范围"), "{md}");
        assert!(md.contains("最近 30 天"), "{md}");
        assert!(md.contains("未参与本次结论"), "{md}");
    }

    /// HTML 导出携带范围说明（自包含报告同样如实标注）。
    #[test]
    fn 导出_html携带范围说明() {
        let mut report = crate::dto::ScanReportDto::from(rb_core::model::report::ScanReport {
            repo_path: "/tmp/demo".into(),
            findings: Vec::new(),
            scores: rb_core::model::report::Scores {
                structure: Some(100),
                history: Some(100),
                branches: Some(100),
                deps: Some(100),
                security: Some(100),
            },
            started_at: 1,
            duration_ms: 2,
            checker_timings: Vec::new(),
            engine_version: "0.1.0".into(),
        });
        report.scope_note = scope_note_of(&ScanConfigDto {
            max_commits: Some(10),
            ..Default::default()
        });
        let html = super::report_to_html(&report);
        assert!(html.contains("class=\"scope-note\""), "缺少范围提示块");
        assert!(html.contains("最新 10 个提交"), "缺少范围文案");
    }
}
