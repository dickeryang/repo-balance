//! DTO 镜像层：rb-core 模型 → camelCase 前端传输对象。
//!
//! 所有类型 `#[serde(rename_all = "camelCase")]`，rb-core 既有模型零改动。
//! Severity/Category 复用 rb-core 原枚举（snake_case 序列化值不变）。

use serde::{Deserialize, Serialize};

use rb_core::model::finding::{Category, Finding, Severity};
use rb_core::model::report::{ScanReport, Scores};
use rb_core::model::snapshot::{CommitSummary, RepoSnapshot};

/// 仓库快照 DTO（camelCase）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshotDto {
    pub path: String,
    pub branch_count: usize,
    pub branches: Vec<String>,
    pub commit_count: usize,
    pub recent_commits: Vec<CommitSummaryDto>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub dep_manifests: Vec<String>,
}

/// 提交摘要 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummaryDto {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub time: i64,
}

/// 单条检查结论 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingDto {
    pub id: String,
    pub severity: Severity,
    pub category: Category,
    pub title: String,
    pub evidence: String,
    pub suggestion: String,
}

/// 五维评分 DTO（None = N/A 维）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoresDto {
    pub structure: Option<u8>,
    pub history: Option<u8>,
    pub branches: Option<u8>,
    pub deps: Option<u8>,
    pub security: Option<u8>,
}

/// 雷达图单维数据点 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarPointDto {
    pub key: String,
    pub name: String,
    pub score: Option<u8>,
    pub finding_count: usize,
}

/// 扫描进度事件 DTO（stub 阶段不产出，待 D3-2 落地后启用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ProgressEventDto {
    pub stage: String,
    pub checker_id: String,
    pub done: usize,
    pub total: usize,
}

/// 完整扫描报告 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReportDto {
    pub repo_path: String,
    pub findings: Vec<FindingDto>,
    pub scores: ScoresDto,
    pub radar_points: Vec<RadarPointDto>,
    pub started_at: i64,
    pub duration_ms: u64,
    pub engine_version: String,
    /// stub 阶段恒为 false；待 D5-1 落地后支持取消扫描的 PartialReport。
    pub cancelled: bool,
}

// ===== From 转换（rb-core 模型 → DTO）=====

impl From<CommitSummary> for CommitSummaryDto {
    fn from(c: CommitSummary) -> Self {
        Self {
            id: c.id,
            summary: c.summary,
            author: c.author,
            time: c.time,
        }
    }
}

impl From<RepoSnapshot> for RepoSnapshotDto {
    fn from(s: RepoSnapshot) -> Self {
        Self {
            path: s.path,
            branch_count: s.branch_count,
            branches: s.branches,
            commit_count: s.commit_count,
            recent_commits: s.recent_commits.into_iter().map(Into::into).collect(),
            file_count: s.file_count,
            total_bytes: s.total_bytes,
            dep_manifests: s.dep_manifests,
        }
    }
}

impl From<Finding> for FindingDto {
    fn from(f: Finding) -> Self {
        Self {
            id: f.id,
            severity: f.severity,
            category: f.category,
            title: f.title,
            evidence: f.evidence,
            suggestion: f.suggestion,
        }
    }
}

impl From<Scores> for ScoresDto {
    fn from(s: Scores) -> Self {
        Self {
            structure: Some(s.structure),
            history: Some(s.history),
            branches: Some(s.branches),
            deps: Some(s.deps),
            security: Some(s.security),
        }
    }
}

/// 维度中文文名表。
const DIM_NAMES: [(Category, &str, &str); 5] = [
    (Category::Structure, "structure", "结构"),
    (Category::History, "history", "历史"),
    (Category::Branches, "branches", "分支"),
    (Category::Deps, "deps", "依赖"),
    (Category::Security, "security", "安全"),
];

/// 从 Scores + findings 组装雷达图数据点。
fn build_radar_points(scores: &Scores, findings: &[Finding]) -> Vec<RadarPointDto> {
    DIM_NAMES
        .iter()
        .map(|(cat, key, name)| {
            let score = match cat {
                Category::Structure => Some(scores.structure),
                Category::History => Some(scores.history),
                Category::Branches => Some(scores.branches),
                Category::Deps => Some(scores.deps),
                Category::Security => Some(scores.security),
            };
            let finding_count = findings.iter().filter(|f| f.category == *cat).count();
            RadarPointDto {
                key: key.to_string(),
                name: name.to_string(),
                score,
                finding_count,
            }
        })
        .collect()
}

impl From<ScanReport> for ScanReportDto {
    fn from(r: ScanReport) -> Self {
        let radar_points = build_radar_points(&r.scores, &r.findings);
        Self {
            repo_path: r.repo_path,
            findings: r.findings.into_iter().map(Into::into).collect(),
            scores: r.scores.into(),
            radar_points,
            started_at: r.started_at,
            duration_ms: r.duration_ms,
            engine_version: r.engine_version,
            cancelled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_dto_字段为_camel_case() {
        let snap = RepoSnapshot {
            path: "/tmp/demo".into(),
            branch_count: 2,
            branches: vec!["main".into()],
            commit_count: 10,
            recent_commits: vec![],
            file_count: 5,
            total_bytes: 1024,
            dep_manifests: vec!["package.json".into()],
        };
        let json = serde_json::to_string(&RepoSnapshotDto::from(snap)).unwrap();
        assert!(json.contains("\"branchCount\""), "{json}");
        assert!(json.contains("\"commitCount\""), "{json}");
        assert!(json.contains("\"fileCount\""), "{json}");
        assert!(json.contains("\"totalBytes\""), "{json}");
        assert!(json.contains("\"depManifests\""), "{json}");
        assert!(json.contains("\"recentCommits\""), "{json}");
    }

    #[test]
    fn scan_report_dto_含_radar_points_与_cancelled() {
        let report = ScanReport {
            repo_path: "/tmp/demo".into(),
            findings: vec![Finding {
                id: "big-files.worktree:a.bin".into(),
                severity: Severity::Warning,
                category: Category::Structure,
                title: "大文件".into(),
                evidence: "5MB".into(),
                suggestion: "加入 gitignore".into(),
            }],
            scores: Scores {
                structure: 80,
                history: 100,
                branches: 100,
                deps: 100,
                security: 100,
            },
            started_at: 1,
            duration_ms: 100,
            engine_version: "0.1.0".into(),
        };
        let dto = ScanReportDto::from(report);
        assert_eq!(dto.radar_points.len(), 5);
        assert!(!dto.cancelled);
        let structure_point = dto
            .radar_points
            .iter()
            .find(|p| p.key == "structure")
            .unwrap();
        assert_eq!(structure_point.score, Some(80));
        assert_eq!(structure_point.finding_count, 1);
        let history_point = dto.radar_points.iter().find(|p| p.key == "history").unwrap();
        assert_eq!(history_point.finding_count, 0);
    }

    #[test]
    fn progress_event_dto_字段为_camel_case() {
        let evt = ProgressEventDto {
            stage: "check".into(),
            checker_id: "big-files".into(),
            done: 1,
            total: 3,
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"checkerId\""), "{json}");
    }
}