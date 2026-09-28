//! CLI 入口：`rb scan <path> [--ci|--json|--quiet]`
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rb_core::checker::{BigFilesChecker, CheckerRegistry};
use rb_core::engine::scan::scan;
use rb_core::git::GitRepo;
use rb_core::model::finding::Severity;
use rb_core::Result;

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
    for flag in args {
        match flag.as_str() {
            "--ci" => ci = true,
            "--json" => json = true,
            "--quiet" => quiet = true,
            other => {
                eprintln!("未知选项: {other}");
                return usage();
            }
        }
    }

    match run_scan(&path, ci, json, quiet) {
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

/// 返回 `Ok(true)` 表示存在 Critical finding（CI 应以退出码 2 标记）。
fn run_scan(path: &Path, ci: bool, json: bool, quiet: bool) -> Result<bool> {
    let snap = GitRepo::open(path)?.snapshot()?;
    let mut registry = CheckerRegistry::new();
    registry.register(Box::new(BigFilesChecker))?;
    let report = scan(path, &registry)?;

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

    if json {
        print_json(&snap, &report, critical_count, warning_count, info_count);
        return Ok(critical_count > 0);
    }

    if ci {
        print_ci(&snap, &report, critical_count, warning_count, info_count);
        return Ok(critical_count > 0);
    }

    if !quiet {
        print_human(&snap, &report);
    }

    Ok(critical_count > 0)
}

fn print_human(snap: &rb_core::model::snapshot::RepoSnapshot, report: &rb_core::model::report::ScanReport) {
    println!("仓衡 (rb) 仓库摘要: {}", snap.path);
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
        println!(
            "  体检结论: 共 {} 条 finding",
            report.findings.len()
        );
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
) {
    let total_score = report.scores.total();
    println!("== 仓衡 RepoBalance CI 报告 ==");
    println!("仓库: {}", snap.path);
    println!("总分: {total_score}/100");
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
) {
    let total_score = report.scores.total();
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
    println!(
        r#"{{"repo":"{}","totalScore":{},"findingCount":{},"critical":{},"warning":{},"info":{},"findings":[{}]}}"#,
        json_escape(&snap.path),
        total_score,
        report.findings.len(),
        critical,
        warning,
        info,
        findings_json.join(",")
    );
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
    eprintln!("用法: rb scan <path> [--ci|--json|--quiet]");
    eprintln!();
    eprintln!("选项:");
    eprintln!("  --ci     CI 模式：简洁输出，Critical 存在时退出码 2");
    eprintln!("  --json   JSON 模式：单行 JSON 输出");
    eprintln!("  --quiet  静默模式：不输出详情，仅按退出码反映结果");
    eprintln!();
    eprintln!("退出码:");
    eprintln!("  0  无 Critical 问题");
    eprintln!("  1  运行错误");
    eprintln!("  2  存在 Critical 问题");
    ExitCode::from(1)
}
