//! CLI 入口：`rb scan <path>`
use std::path::Path;

use rb_core::error::Result;
use rb_core::git::GitRepo;

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = match args.next() {
        Some(c) => c,
        None => return usage(),
    };
    if cmd == "--help" || cmd == "-h" {
        return usage();
    }
    if cmd != "scan" {
        eprintln!("未知命令: {cmd}");
        return usage();
    }
    let path = match args.next() {
        Some(p) => Path::new(&p).to_path_buf(),
        None => {
            eprintln!("缺少 <path> 参数");
            return usage();
        }
    };

    match run_scan(&path) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("错误: {e}");
            std::process::exit(1);
        }
    }
}

fn run_scan(path: &Path) -> Result<()> {
    let repo = GitRepo::open(path)?;
    let snap = repo.snapshot()?;
    println!("仓衡 (rb) 仓库摘要: {}", snap.path);
    println!("  提交数: {}", snap.commit_count);
    println!("  分支数: {} ({})", snap.branch_count, snap.branches.join(", "));
    println!("  文件数: {}", snap.file_count);
    println!("  总大小: {} bytes", snap.total_bytes);
    if !snap.dep_manifests.is_empty() {
        println!("  依赖清单:");
        for m in &snap.dep_manifests {
            println!("    - {m}");
        }
    }
    if let Some(latest) = snap.recent_commits.first() {
        println!("  最新提交: {} {}", &latest.id[..12.min(latest.id.len())], latest.summary);
    }
    Ok(())
}

fn usage() {
    eprintln!("用法: rb scan <path>");
}
