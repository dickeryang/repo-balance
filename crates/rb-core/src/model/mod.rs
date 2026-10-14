pub mod finding;
pub mod report;
pub mod scale;
pub mod slice_meta;
pub mod snapshot;
pub mod vuln;
#[cfg(test)]
#[allow(non_snake_case)]
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
                time_offset: 0,
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
                structure: Some(90),
                history: Some(80),
                branches: None,
                deps: Some(60),
                security: Some(50),
            },
            started_at: 1,
            duration_ms: 2,
            checker_timings: Vec::new(),
            engine_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: ScanReport = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    /// Scores.total() 只平均已覆盖维度；未覆盖维度不参与、不拖低总分。
    #[test]
    fn scores_total_只平均已覆盖维度() {
        let s = Scores {
            structure: Some(90),
            history: Some(80),
            branches: None,
            deps: None,
            security: None,
        };
        assert_eq!(s.total(), Some(85));
    }

    /// 全部维度未覆盖时 total() 为 None（空注册表等场景）。
    #[test]
    fn scores_total_全部未覆盖_返回None() {
        let s = Scores {
            structure: None,
            history: None,
            branches: None,
            deps: None,
            security: None,
        };
        assert_eq!(s.total(), None);
    }

    /// 枚举非法取值在反序列化边界报错。
    #[test]
    fn invalid_enum_value_fails_deserialization() {
        assert!(serde_json::from_str::<Severity>("\"fatal\"").is_err());
        assert!(serde_json::from_str::<Category>("\"style\"").is_err());
    }
}
