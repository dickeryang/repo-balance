//! CLI 入口：`rb scan <path>`
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

    match run_scan(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_scan(path: &Path) -> Result<()> {
    // 仓库摘要（不回退 workspace_init 任务 7 的摘要可见性）。
    let snap = GitRepo::open(path)?.snapshot()?;
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

    // 经 engine 扫描产出 findings。
    let mut registry = CheckerRegistry::new();
    registry.register(Box::new(BigFilesChecker))?;
    let report = scan(path, &registry)?;

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
    Ok(())
}

/// 用法提示路径统一为非零退出码。
fn usage() -> ExitCode {
    eprintln!("用法: rb scan <path>");
    ExitCode::from(1)
}
