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

export type CheckerStatus = 'pending' | 'running' | 'done'

export interface CheckerFeedRow {
  checkerId: string
  status: CheckerStatus
  findingCount: number
}