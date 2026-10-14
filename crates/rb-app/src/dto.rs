//! DTO 镜像层：rb-core 模型 → camelCase 前端传输对象。
//!
//! 所有类型 `#[serde(rename_all = "camelCase")]`，rb-core 既有模型零改动。
//! Severity/Category 复用 rb-core 原枚举（snake_case 序列化值不变）。

use serde::{Deserialize, Serialize};

use rb_core::checker::ScanConfig;
use rb_core::model::finding::{Category, Finding, Severity};
use rb_core::model::report::{ScanReport, Scores};
use rb_core::model::scale::{ScaleEstimate, ScaleLevel};
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

/// 扫描配置 DTO（camelCase）：用户可调整阈值、历史范围与启用检查器集合。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfigDto {
    /// 大文件判定阈值（字节），默认 1 MiB。
    pub big_file_threshold: u64,
    /// 僵尸分支「超 N 天未活跃」阈值（天），默认 90。
    #[serde(default = "default_zombie_branch_days")]
    pub zombie_branch_days: u64,
    /// 启用的检查器 id 列表（空表示全部启用）。
    pub enabled_checkers: Vec<String>,
    /// 历史遍历提交数上限（保留最新的若干个）；`null` 表示不限。
    #[serde(default)]
    pub max_commits: Option<usize>,
    /// 历史时间窗（天，仅遍历最近 N 天的提交）；`null` 表示不限。
    ///
    /// DTO 用「天数」而非 Unix 秒：GUI 更易理解，秒级换算在命令层完成。
    #[serde(default)]
    pub since_days: Option<u64>,
}

/// 规模档位 DTO（与 [`ScaleLevel`] 一一对应，序列化值 snake_case）。
pub type ScaleLevelDto = ScaleLevel;

/// 扫描规模预估 DTO（camelCase 镜像 [`ScaleEstimate`]）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleEstimateDto {
    pub commit_count: usize,
    pub file_count: usize,
    pub total_bytes: u64,
    pub avg_objects_per_commit: f64,
    pub estimated_objects: u64,
    pub estimated_seconds: u32,
    pub level: ScaleLevelDto,
    pub suggested_max_commits: Option<usize>,
    pub suggested_since_days: Option<u64>,
}

impl From<ScaleEstimate> for ScaleEstimateDto {
    fn from(e: ScaleEstimate) -> Self {
        Self {
            commit_count: e.commit_count,
            file_count: e.file_count,
            total_bytes: e.total_bytes,
            avg_objects_per_commit: e.avg_objects_per_commit,
            estimated_objects: e.estimated_objects,
            estimated_seconds: e.estimated_seconds,
            level: e.level,
            suggested_max_commits: e.suggested_max_commits,
            suggested_since_days: e.suggested_since_days,
        }
    }
}

/// [`ScanConfigDto::zombie_branch_days`] 的 serde 缺省值（90 天）。
fn default_zombie_branch_days() -> u64 {
    90
}

impl Default for ScanConfigDto {
    fn default() -> Self {
        Self {
            big_file_threshold: 1_048_576,
            zombie_branch_days: default_zombie_branch_days(),
            enabled_checkers: ALL_CHECKER_IDS.iter().map(|s| s.to_string()).collect(),
            max_commits: None,
            since_days: None,
        }
    }
}

/// 一天的秒数（`since_days` → `history_since` 换算用）。
const SECS_PER_DAY: i64 = 86_400;

impl ScanConfigDto {
    /// 转换为 rb-core 配置：`since_days` 按 `now`（Unix 秒）换算为绝对时间戳。
    ///
    /// 0 值按「不限」处理——GUI 输入框清空或误填 0 不应导致扫描出空历史。
    pub fn to_core_config(&self, now: i64) -> ScanConfig {
        ScanConfig {
            big_file_threshold: self.big_file_threshold,
            zombie_branch_days: self.zombie_branch_days,
            max_commits: self.max_commits.filter(|n| *n > 0),
            // 漏洞库由壳层在命令层注入（见 commands.rs），DTO 不参与传输。
            vuln_index: None,
            history_since: self
                .since_days
                .filter(|d| *d > 0)
                .map(|d| now - d as i64 * SECS_PER_DAY),
        }
    }
}

/// 全部内置检查器 id（与 rb-core 注册顺序一致）。
pub const ALL_CHECKER_IDS: [&str; 6] = [
    "big-files",
    "secrets",
    "conflict-markers",
    "zombie-branches",
    "commit-habits",
    "deps-health",
];

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
    /// 取消的扫描推送 `cancelled=true` 的部分报告，前端照常渲染「部分结果」。
    pub cancelled: bool,
    /// 历史范围受限时的告知文案；全量扫描为 `None`。
    ///
    /// 由命令层按当次扫描配置生成，报告页与导出（MD/HTML）均需如实标注，
    /// 避免「受限结论」被误读为「全量结论」。
    #[serde(default)]
    pub scope_note: Option<String>,
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
            structure: s.structure,
            history: s.history,
            branches: s.branches,
            deps: s.deps,
            security: s.security,
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
                Category::Structure => scores.structure,
                Category::History => scores.history,
                Category::Branches => scores.branches,
                Category::Deps => scores.deps,
                Category::Security => scores.security,
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
            scope_note: None,
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
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
                structure: Some(80),
                history: Some(100),
                branches: Some(100),
                deps: Some(100),
                security: Some(100),
            },
            started_at: 1,
            duration_ms: 100,
            checker_timings: Vec::new(),
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
        let history_point = dto
            .radar_points
            .iter()
            .find(|p| p.key == "history")
            .unwrap();
        assert_eq!(history_point.finding_count, 0);
    }

    /// 未覆盖维度（None）在雷达点上保持 N/A 而非 0。
    #[test]
    fn scan_report_dto_未覆盖维度为None() {
        let report = ScanReport {
            repo_path: "/tmp/demo".into(),
            findings: vec![],
            scores: Scores {
                structure: Some(80),
                history: None,
                branches: None,
                deps: None,
                security: None,
            },
            started_at: 1,
            duration_ms: 100,
            checker_timings: Vec::new(),
            engine_version: "0.1.0".into(),
        };
        let dto = ScanReportDto::from(report);
        let history_point = dto
            .radar_points
            .iter()
            .find(|p| p.key == "history")
            .unwrap();
        assert_eq!(history_point.score, None);
        assert_eq!(dto.scores.history, None);
        assert_eq!(dto.scores.structure, Some(80));
    }

    /// ScanConfigDto 默认启用全部六个检查器、僵尸分支阈值 90 天、历史不限。
    #[test]
    fn scan_config_dto_默认启用全部检查器() {
        let cfg = ScanConfigDto::default();
        assert_eq!(cfg.big_file_threshold, 1_048_576);
        assert_eq!(cfg.zombie_branch_days, 90);
        assert_eq!(cfg.enabled_checkers.len(), ALL_CHECKER_IDS.len());
        assert_eq!(cfg.max_commits, None, "默认不限历史提交数");
        assert_eq!(cfg.since_days, None, "默认不限历史时间窗");
        for id in ALL_CHECKER_IDS {
            assert!(
                cfg.enabled_checkers.contains(&id.to_owned()),
                "缺少检查器 {id}"
            );
        }
    }

    /// ScanConfigDto 反序列化缺省 zombieBranchDays 时回退 90。
    #[test]
    fn scan_config_dto_反序列化缺省回退() {
        let cfg: ScanConfigDto =
            serde_json::from_str(r#"{"bigFileThreshold":2048,"enabledCheckers":["big-files"]}"#)
                .unwrap();
        assert_eq!(cfg.big_file_threshold, 2048);
        assert_eq!(cfg.zombie_branch_days, 90);
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"zombieBranchDays\""), "{json}");
    }

    /// 历史范围字段为 camelCase（maxCommits / sinceDays）。
    #[test]
    fn scan_config_dto_历史范围字段为_camel_case() {
        let cfg = ScanConfigDto {
            max_commits: Some(500),
            since_days: Some(365),
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"maxCommits\":500"), "{json}");
        assert!(json.contains("\"sinceDays\":365"), "{json}");
    }

    /// 旧配置（无历史范围字段）反序列化回退为不限。
    #[test]
    fn scan_config_dto_旧配置缺省历史范围() {
        let cfg: ScanConfigDto =
            serde_json::from_str(r#"{"bigFileThreshold":1,"enabledCheckers":[]}"#).unwrap();
        assert_eq!(cfg.max_commits, None);
        assert_eq!(cfg.since_days, None);
    }

    /// `since_days` 按 now 换算为绝对时间戳；0 与 None 均为「不限」。
    #[test]
    fn to_core_config_时间窗换算与零值归一化() {
        let base = ScanConfigDto::default();
        assert_eq!(base.to_core_config(1_700_000_000).history_since, None);
        assert_eq!(base.to_core_config(1_700_000_000).max_commits, None);

        let limited = ScanConfigDto {
            max_commits: Some(500),
            since_days: Some(10),
            ..Default::default()
        };
        let core = limited.to_core_config(1_700_000_000);
        assert_eq!(core.max_commits, Some(500));
        assert_eq!(core.history_since, Some(1_700_000_000 - 10 * 86_400));

        let zero = ScanConfigDto {
            max_commits: Some(0),
            since_days: Some(0),
            ..Default::default()
        };
        let core = zero.to_core_config(1_700_000_000);
        assert_eq!(core.max_commits, None, "0 按不限处理");
        assert_eq!(core.history_since, None, "0 按不限处理");
    }

    /// ScaleEstimateDto 字段为 camelCase，档位为 snake_case。
    #[test]
    fn scale_estimate_dto_字段为_camel_case() {
        let est = ScaleEstimate {
            commit_count: 12,
            file_count: 3,
            total_bytes: 42,
            avg_objects_per_commit: 2.5,
            estimated_objects: 30,
            estimated_seconds: 2,
            level: ScaleLevel::Small,
            suggested_max_commits: None,
            suggested_since_days: None,
        };
        let json = serde_json::to_string(&ScaleEstimateDto::from(est)).unwrap();
        for key in [
            "\"commitCount\"",
            "\"fileCount\"",
            "\"totalBytes\"",
            "\"avgObjectsPerCommit\"",
            "\"estimatedObjects\"",
            "\"estimatedSeconds\"",
            "\"suggestedMaxCommits\"",
            "\"suggestedSinceDays\"",
            "\"level\":\"small\"",
        ] {
            assert!(json.contains(key), "缺少字段 {key}: {json}");
        }
    }
}
