export type Severity = 'critical' | 'warning' | 'info'
export type Category = 'structure' | 'history' | 'branches' | 'deps' | 'security'

export interface CommitSummary {
  id: string
  summary: string
  author: string
  time: number
}

export interface RepoSnapshot {
  path: string
  branchCount: number
  branches: string[]
  commitCount: number
  recentCommits: CommitSummary[]
  fileCount: number
  totalBytes: number
  depManifests: string[]
}

export interface Finding {
  id: string
  severity: Severity
  category: Category
  title: string
  evidence: string
  suggestion: string
}

export interface Scores {
  structure: number | null
  history: number | null
  branches: number | null
  deps: number | null
  security: number | null
}

export interface RadarPoint {
  key: string
  name: string
  score: number | null
  findingCount: number
}

export interface ScanReport {
  repoPath: string
  findings: Finding[]
  scores: Scores
  radarPoints: RadarPoint[]
  startedAt: number
  durationMs: number
  engineVersion: string
  cancelled: boolean
}

export interface ProgressEvent {
  stage: string
  checkerId?: string
  done: number
  total: number
  /** 0-100 估算进度百分比（后端按阶段统一计算，历史采集阶段按提交比例推进）。 */
  percent?: number
}

/** 扫描配置：用户可调整大文件阈值与启用检查器集合。 */
export interface ScanConfig {
  /** 大文件判定阈值（字节）。 */
  bigFileThreshold: number
  /** 启用的检查器 id 列表（空表示全部启用）。 */
  enabledCheckers: string[]
}

export type CheckerStatus = 'pending' | 'running' | 'done'

export interface CheckerFeedRow {
  checkerId: string
  status: CheckerStatus
  findingCount: number
}

/** 扫描历史条目：保存报告快照用于跨次对比。 */
export interface HistoryEntry {
  /** 唯一 id（时间戳 + 随机后缀）。 */
  id: string
  /** 保存时间（epoch ms）。 */
  savedAt: number
  /** 仓库路径。 */
  repoPath: string
  /** 仓库名（路径末段）。 */
  repoName: string
  /** 总分（0-100）。 */
  totalScore: number
  /** 发现项数量。 */
  findingCount: number
  /** 完整报告快照。 */
  report: ScanReport
}

/** 两次扫描的对比结果。 */
export interface HistoryCompare {
  /** 基线条目。 */
  baseline: HistoryEntry
  /** 对照条目。 */
  target: HistoryEntry
  /** 总分变化（target - baseline）。 */
  scoreDelta: number
  /** 发现项数量变化。 */
  findingCountDelta: number
  /** 新增的发现项（target 中有、baseline 中无，按 id 匹配）。 */
  addedFindings: Finding[]
  /** 消除的发现项（baseline 中有、target 中无）。 */
  resolvedFindings: Finding[]
  /** 两轮都存在的发现项。 */
  commonFindings: Finding[]
}