pub mod finding;
pub mod report;
pub mod slice_meta;
pub mod snapshot;
#[cfg(test)]
mod tests {
    use super::finding::{Category, Finding, Severity};
    use super::report::{ScanReport, Scores};
    use super::snapshot::{CommitSummary, RepoSnapshot};

    fn sample_snapshot() -> RepoSnapshot {
        RepoSnapshot {
            path: "/tmp/demo".into(),
            branch_count: 2,
            branches: vec!["dev".into(), "main".into()],
            commit_count: 30,
            recent_commits: vec![CommitSummary {
                id: "a".repeat(40),
                summary: "init".into(),
                author: "tester".into(),
                time: 1_700_000_000,
            }],
            file_count: 3,
            total_bytes: 1024,
            dep_manifests: vec!["package.json".into()],
        }
    }

    /// RepoSnapshot 的 JSON 序列化与反序列化往返等价（spec 5.2.1.3）。
    #[test]
    fn snapshot_json_roundtrip_is_stable() {
        let snap = sample_snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let back: RepoSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    /// 字段名 snake_case 稳定。
    #[test]
    fn snapshot_fields_are_snake_case() {
        let json = serde_json::to_string(&sample_snapshot()).unwrap();
        for key in [
            "branch_count",
            "commit_count",
            "recent_commits",
            "file_count",
            "total_bytes",
            "dep_manifests",
        ] {
            assert!(json.contains(key), "缺少字段 {key}");
        }
    }

    /// Finding 的 JSON 往返等价，枚举输出 snake_case。
    #[test]
    fn finding_json_roundtrip_is_stable() {
        let f = Finding {
            id: "history.merge-commit-ratio".into(),
            severity: Severity::Warning,
            category: Category::History,
            title: "t".into(),
            evidence: "e".into(),
            suggestion: "s".into(),
        };
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"severity\":\"warning\""), "{json}");
        assert!(json.contains("\"category\":\"history\""), "{json}");
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    /// ScanReport 的 JSON 往返等价。
    #[test]
    fn scan_report_json_roundtrip_is_stable() {
        let r = ScanReport {
            repo_path: "/tmp/demo".into(),
            findings: Vec::new(),
            scores: Scores {
                structure: 90,
                history: 80,
                branches: 70,
                deps: 60,
                security: 50,
            },
            started_at: 1,
            duration_ms: 2,
            engine_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: ScanReport = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    /// 枚举非法取值在反序列化边界报错。
    #[test]
    fn invalid_enum_value_fails_deserialization() {
        assert!(serde_json::from_str::<Severity>("\"fatal\"").is_err());
        assert!(serde_json::from_str::<Category>("\"style\"").is_err());
    }
}
