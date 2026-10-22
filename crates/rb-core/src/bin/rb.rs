//! CLI 入口：
//! `rb scan <path> [--ci|--json|--quiet] [--max-commits N] [--since-days N] [--vuln-db FILE] [--rules FILE]`
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rb_core::checker::{
    BigFilesChecker, CheckerRegistry, CommitHabitsChecker, ConflictMarkersChecker,
    CustomRulesChecker, DepsHealthChecker, SecretsChecker, ZombieBranchesChecker,
};
use rb_core::engine::scan::scan_with_progress;
use rb_core::git::GitRepo;
use rb_core::model::finding::Severity;
use rb_core::rules::load_rules_json;
use rb_core::{RbError, Result};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        return usage();
    };
    if cmd == "--help" || cmd == "-h" {
        return usage();
    }
    if cmd == "ignore" {
        let Some(p) = args.next() else {
            eprintln!("缺少 <path> 参数");
            return usage();
        };
        return cmd_ignore(Path::new(&p));
    }
    if cmd != "scan" && cmd != "scan-url" {
        eprintln!("未知命令: {cmd}");
        return usage();
    }
    let is_remote = cmd == "scan-url";
    let Some(p) = args.next() else {
        eprintln!("缺少 <path|url> 参数");
        return usage();
    };
    let path = PathBuf::from(p);

    let mut ci = false;
    let mut json = false;
    let mut quiet = false;
    // 大仓库性能闸门：`--max-commits N` 只遍历最新 N 个提交；
    // `--since-days N` 只遍历最近 N 天的提交。两者可叠加（取交集）。
    let mut max_commits: Option<usize> = None;
    let mut since_days: Option<u64> = None;
    // 离线漏洞库索引路径（JSONL）；不指定时不做漏洞匹配。
    let mut vuln_db: Option<PathBuf> = None;
    // 自定义规则文件（JSON 数组）；不指定时不启用自定义规则检查器。
    let mut rules_file: Option<PathBuf> = None;
    // 远程体检浅克隆深度（仅 scan-url 生效）。
    let mut depth: i32 = rb_core::git::DEFAULT_CLONE_DEPTH;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ci" => ci = true,
            "--json" => json = true,
            "--quiet" => quiet = true,
            "--depth" => {
                let Some(raw) = args.next() else {
                    eprintln!("--depth 需要数字参数");
                    return usage();
                };
                match raw.parse::<i32>() {
                    Ok(0) => {
                        eprintln!("--depth 必须大于 0");
                        return usage();
                    }
                    Ok(n) => depth = n,
                    Err(_) => {
                        eprintln!("--depth 参数非法: {raw}");
                        return usage();
                    }
                }
            }
            "--max-commits" => {
                let Some(raw) = args.next() else {
                    eprintln!("--max-commits 需要数字参数");
                    return usage();
                };
                match raw.parse::<usize>() {
                    Ok(0) => {
                        eprintln!("--max-commits 必须大于 0");
                        return usage();
                    }
                    Ok(n) => max_commits = Some(n),
                    Err(_) => {
                        eprintln!("--max-commits 参数非法: {raw}");
                        return usage();
                    }
                }
            }
            "--since-days" => {
                let Some(raw) = args.next() else {
                    eprintln!("--since-days 需要数字参数");
                    return usage();
                };
                match raw.parse::<u64>() {
                    Ok(0) => {
                        eprintln!("--since-days 必须大于 0");
                        return usage();
                    }
                    Ok(n) => since_days = Some(n),
                    Err(_) => {
                        eprintln!("--since-days 参数非法: {raw}");
                        return usage();
                    }
                }
            }
            "--vuln-db" => {
                let Some(raw) = args.next() else {
                    eprintln!("--vuln-db 需要文件路径参数");
                    return usage();
                };
                vuln_db = Some(PathBuf::from(raw));
            }
            "--rules" => {
                let Some(raw) = args.next() else {
                    eprintln!("--rules 需要文件路径参数");
                    return usage();
                };
                rules_file = Some(PathBuf::from(raw));
            }
            other => {
                eprintln!("未知选项: {other}");
                return usage();
            }
        }
    }

    let options = ScanOptions {
        ci,
        json,
        quiet,
        max_commits,
        since_days,
        vuln_db,
        rules_file,
    };
    let outcome = if is_remote {
        // 远程体检：浅克隆到临时目录后复用本地扫描管线；TempDir RAII
        // 保证扫描结束（含出错路径）后清理克隆产物，不落盘残留。
        match tempfile::tempdir() {
            Ok(tmp) => {
                eprintln!("浅克隆 {} (depth={depth}) …", path.display());
                match rb_core::git::clone_shallow(
                    path.to_string_lossy().as_ref(),
                    tmp.path(),
                    depth,
                ) {
                    Ok(()) => {
                        eprintln!("克隆完成，开始扫描（临时目录退出后自动清理）");
                        run_scan(tmp.path(), &options)
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(rb_core::RbError::Io(e)),
        }
    } else {
        run_scan(&path, &options)
    };
    match outcome {
        Ok(has_critical) => {
            if has_critical {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("错误: {e}");
            ExitCode::from(1)
        }
    }
}

/// 命令行扫描选项（聚成结构体，避免参数列表随选项增加而膨胀）。
struct ScanOptions {
    /// CI 模式：简洁输出。
    ci: bool,
    /// JSON 模式：单行机器可读输出。
    json: bool,
    /// 静默模式：不输出详情，仅以退出码反映结果。
    quiet: bool,
    /// 大仓库性能闸门：只遍历最新 N 个提交；None 表示不限。
    max_commits: Option<usize>,
    /// 大仓库性能闸门：只遍历最近 N 天的提交；None 表示不限。
    since_days: Option<u64>,
    /// 离线漏洞库索引（JSONL）路径；None 时依赖维度仅做结构检查。
    vuln_db: Option<PathBuf>,
    /// 自定义规则（JSON 数组）路径；None 时不注册自定义规则检查器。
    rules_file: Option<PathBuf>,
}

/// 返回 `Ok(true)` 表示存在 Critical finding（CI 应以退出码 2 标记）。
fn run_scan(path: &Path, options: &ScanOptions) -> Result<bool> {
    let ScanOptions {
        ci,
        json,
        quiet,
        max_commits,
        since_days,
        vuln_db,
        rules_file,
    } = options;
    let (ci, json, quiet) = (*ci, *json, *quiet);
    let (max_commits, since_days) = (*max_commits, *since_days);
    let snap = GitRepo::open(path)?.snapshot()?;
    let mut registry = CheckerRegistry::new();
    registry.register(Box::new(BigFilesChecker))?;
    registry.register(Box::new(SecretsChecker))?;
    registry.register(Box::new(ConflictMarkersChecker))?;
    registry.register(Box::new(ZombieBranchesChecker))?;
    registry.register(Box::new(CommitHabitsChecker))?;
    registry.register(Box::new(DepsHealthChecker))?;

    // 自定义规则：文件读取失败 / JSON 非法 / 正则无法编译一律快速失败——
    // 规则是用户显式配置，静默降级会让人误以为规则已生效。
    let custom_rules = match rules_file {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .map_err(|e| RbError::InvalidRule(format!("读取 {}: {e}", p.display())))?;
            load_rules_json(&text)?
        }
        None => Vec::new(),
    };
    // 无规则不注册：否则会把「没有任何规则的维度」错误标记为已覆盖（假满分）。
    let rules_count = custom_rules.len();
    if !custom_rules.is_empty() {
        registry.register(Box::new(CustomRulesChecker::new(custom_rules)?))?;
    }

    let history_since = since_days.map(|d| now_secs() - d as i64 * 86_400);
    // 漏洞库缺失时不做匹配（依赖维度退化为结构检查），不因可选增强失败而中断扫描。
    let vuln_index = match vuln_db {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .map_err(|e| rb_core::RbError::VulnDb(format!("读取 {}: {e}", p.display())))?;
            Some(std::sync::Arc::new(rb_core::vuln::load_index(&text)?))
        }
        None => None,
    };
    let config = rb_core::checker::ScanConfig {
        max_commits,
        history_since,
        vuln_index,
        ..Default::default()
    };
    let report = scan_with_progress(path, &registry, None, None, &config)?;

    let critical_count = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Critical)
        .count();
    let warning_count = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let info_count = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();

    // 限流必须显式告知：否则受限结论会被误读为全量结论。
    let scope = scope_note(max_commits, since_days);

    if json {
        print_json(
            &snap,
            &report,
            critical_count,
            warning_count,
            info_count,
            scope.as_deref(),
        );
        return Ok(critical_count > 0);
    }

    if ci {
        print_ci(
            &snap,
            &report,
            critical_count,
            warning_count,
            info_count,
            scope.as_deref(),
        );
        return Ok(critical_count > 0);
    }

    if !quiet {
        print_human(&snap, &report, scope.as_deref(), rules_count);
    }

    Ok(critical_count > 0)
}

/// 历史范围受限时的告知文案；未限流返回 `None`。
fn scope_note(max_commits: Option<usize>, since_days: Option<u64>) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(n) = max_commits {
        parts.push(format!("最新 {n} 个提交"));
    }
    if let Some(d) = since_days {
        parts.push(format!("最近 {d} 天"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!(
        "历史范围: {}（更老的历史未参与本次结论）",
        parts.join(" · ")
    ))
}

fn print_human(
    snap: &rb_core::model::snapshot::RepoSnapshot,
    report: &rb_core::model::report::ScanReport,
    scope: Option<&str>,
    rules_count: usize,
) {
    if let Some(s) = scope {
        println!("  ⚠ {s}");
    }
    println!("仓衡 (rb) 仓库摘要: {}", snap.path);
    if rules_count > 0 {
        println!("  自定义规则: {rules_count} 条已启用");
    }
    println!("  提交数: {}", snap.commit_count);
    println!(
        "  分支数: {} ({})",
        snap.branch_count,
        snap.branches.join(", ")
    );
    println!("  文件数: {}", snap.file_count);
    println!("  总大小: {} bytes", snap.total_bytes);
    if !snap.dep_manifests.is_empty() {
        println!("  依赖清单:");
        for m in &snap.dep_manifests {
            println!("    - {m}");
        }
    }

    if report.findings.is_empty() {
        println!("  体检结论: 未发现需要关注的问题");
    } else {
        println!("  体检结论: 共 {} 条 finding", report.findings.len());
        for f in &report.findings {
            let tag = match f.severity {
                Severity::Critical => "[Critical]",
                Severity::Warning => "[Warning] ",
                Severity::Info => "[Info]    ",
            };
            println!("  {tag} {} — {}", f.id, f.title);
            for line in f.evidence.lines() {
                println!("        {line}");
            }
            println!("        建议: {}", f.suggestion);
        }
    }
}

fn print_ci(
    snap: &rb_core::model::snapshot::RepoSnapshot,
    report: &rb_core::model::report::ScanReport,
    critical: usize,
    warning: usize,
    info: usize,
    scope: Option<&str>,
) {
    let total_score = report
        .scores
        .total()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "N/A".to_owned());
    println!("== 仓衡 RepoBalance CI 报告 ==");
    println!("仓库: {}", snap.path);
    if let Some(s) = scope {
        println!("范围: {s}");
    }
    println!("总分: {total_score}/100（已覆盖维度平均）");
    println!(
        "发现: {} 条 (Critical={critical}, Warning={warning}, Info={info})",
        report.findings.len()
    );
    if !report.findings.is_empty() {
        println!("---");
        for f in &report.findings {
            let tag = match f.severity {
                Severity::Critical => "CRITICAL",
                Severity::Warning => "WARNING",
                Severity::Info => "INFO",
            };
            println!("[{tag}] {f_id}: {title}", f_id = f.id, title = f.title);
        }
    }
    if critical > 0 {
        eprintln!("CI FAIL: 存在 {critical} 条 Critical 问题");
    } else {
        println!("CI PASS: 无 Critical 问题");
    }
}

fn print_json(
    snap: &rb_core::model::snapshot::RepoSnapshot,
    report: &rb_core::model::report::ScanReport,
    critical: usize,
    warning: usize,
    info: usize,
    scope: Option<&str>,
) {
    let total_score = report
        .scores
        .total()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let findings_json: Vec<String> = report
        .findings
        .iter()
        .map(|f| {
            let sev = match f.severity {
                Severity::Critical => "critical",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            format!(
                r#"{{"id":"{}","severity":"{}","title":"{}"}}"#,
                json_escape(&f.id),
                sev,
                json_escape(&f.title)
            )
        })
        .collect();
    // scope 为 null（全量）或字符串（受限文案），供 CI 侧如实记录本次覆盖范围。
    let scope_json = match scope {
        Some(s) => format!("\"{}\"", json_escape(s)),
        None => "null".to_owned(),
    };
    // 各检查器 check 阶段耗时（camelCase 键对齐 totalScore/findingCount 风格）。
    let timings_json: Vec<String> = report
        .checker_timings
        .iter()
        .map(|t| {
            format!(
                r#"{{"checkerId":"{}","durationMs":{}}}"#,
                json_escape(&t.checker_id),
                t.duration_ms
            )
        })
        .collect();
    println!(
        r#"{{"repo":"{}","totalScore":{},"findingCount":{},"critical":{},"warning":{},"info":{},"scope":{},"checkerTimings":[{}],"findings":[{}]}}"#,
        json_escape(&snap.path),
        total_score,
        report.findings.len(),
        critical,
        warning,
        info,
        scope_json,
        timings_json.join(","),
        findings_json.join(",")
    );
}

/// 当前 Unix 秒（系统调用失败时回退 0——仅用于时间窗换算，不参与结论）。
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// 用法提示路径统一为非零退出码。
/// `rb ignore <path>`：只读校验仓库根的忽略规则（不写入、不改动仓库）。
///
/// 与 GUI 编辑器「试算」的静态检查同一口径：把「写了却不会生效」的规则
/// 在进入版本库之前暴露出来（这类规则不会报错，只会静默失效）。
fn cmd_ignore(path: &Path) -> ExitCode {
    let file = path.join(rb_core::ignore::IGNORE_FILE_NAME);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "{} 未找到：当前仅 {} 条内置默认忽略生效",
                rb_core::ignore::IGNORE_FILE_NAME,
                rb_core::ignore::default_patterns().len()
            );
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("读取 {} 失败: {e}", rb_core::ignore::IGNORE_FILE_NAME);
            return ExitCode::FAILURE;
        }
    };

    let user_rules = text
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .count();
    println!(
        "{}：{} 条用户规则 + {} 条内置默认忽略",
        file.display(),
        user_rules,
        rb_core::ignore::default_patterns().len()
    );

    let lints = rb_core::ignore::lint(&text);
    if lints.is_empty() {
        println!("规则检查通过：没有「写了却不会生效」的规则");
        return ExitCode::SUCCESS;
    }
    for l in &lints {
        let why = match l.kind {
            rb_core::ignore::IgnoreLintKind::EmptyPattern => "不含有效路径段，永不匹配任何文件",
            rb_core::ignore::IgnoreLintKind::Backslash => "使用反斜杠分隔，请改用正斜杠 /",
            rb_core::ignore::IgnoreLintKind::Duplicate => "与前面某条规则重复，前面那条无实际效果",
        };
        eprintln!("第 {} 行 `{}`：{why}", l.line, l.pattern);
    }
    eprintln!("共 {} 处可疑规则", lints.len());
    ExitCode::FAILURE
}

fn usage() -> ExitCode {
    eprintln!("用法: rb scan <path> [选项]");
    eprintln!("      rb scan-url <url> [选项]");
    eprintln!("      rb ignore <path>");
    eprintln!();
    eprintln!("选项:");
    eprintln!("  --ci              CI 模式：简洁输出，Critical 存在时退出码 2");
    eprintln!("  --json            JSON 模式：单行 JSON 输出");
    eprintln!("  --quiet           静默模式：不输出详情，仅按退出码反映结果");
    eprintln!("  --max-commits N   只遍历最新 N 个提交（大仓库提速，默认不限）");
    eprintln!("  --since-days N    只遍历最近 N 天的提交（大仓库提速，默认不限）");
    eprintln!("  --vuln-db FILE    离线漏洞库索引（JSONL）；不指定则不做漏洞匹配");
    eprintln!("  --rules FILE      自定义规则（JSON 数组）；不指定则不启用自定义规则");
    eprintln!();
    eprintln!("scan-url 专用:");
    eprintln!("  --depth N         浅克隆深度（默认 1，即只取最新一次提交）");
    eprintln!();
    eprintln!("退出码（scan / scan-url）:");
    eprintln!("  0  无 Critical 问题");
    eprintln!("  1  运行错误");
    eprintln!("  2  存在 Critical 问题");
    eprintln!();
    eprintln!("退出码（rb ignore）:");
    eprintln!("  0  规则检查通过（或仓库未配置忽略规则）");
    eprintln!("  1  存在「写了却不会生效」的可疑规则 / 读取失败");
    ExitCode::from(1)
}
