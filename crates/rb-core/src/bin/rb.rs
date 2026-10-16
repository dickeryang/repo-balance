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
    if cmd != "scan" {
        eprintln!("未知命令: {cmd}");
        return usage();
    }
    let Some(p) = args.next() else {
        eprintln!("缺少 <path> 参数");
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
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--ci" => ci = true,
            "--json" => json = true,
            "--quiet" => quiet = true,
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
    match run_scan(&path, &options) {
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
fn usage() -> ExitCode {
    eprintln!("用法: rb scan <path> [选项]");
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
    eprintln!("退出码:");
    eprintln!("  0  无 Critical 问题");
    eprintln!("  1  运行错误");
    eprintln!("  2  存在 Critical 问题");
    ExitCode::from(1)
}
