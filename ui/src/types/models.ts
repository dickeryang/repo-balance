export type Severity = 'critical' | 'warning' | 'info'

/** 批量体检中单个仓库的状态。 */
export type BatchStatus = 'pending' | 'running' | 'done' | 'skipped' | 'failed'

/** 批量体检队列中的一项（一个仓库一行）。 */
export interface BatchItem {
  /** 仓库路径（本地绝对路径）。 */
  path: string
  /** 仓库名（路径末段）。 */
  name: string
  /** 当前状态。 */
  status: BatchStatus
  /** 总分（已覆盖维度平均）；未完成 / 失败为 null。 */
  total: number | null
  /** 问题总数。 */
  findingCount: number
  /** 严重问题数。 */
  criticalCount: number
  /** 本仓库扫描用时（毫秒）。 */
  durationMs: number
  /** 失败原因（status=failed 时）。 */
  error: string | null
  /** 完整报告（仅 done 保留，供「查看报告」复用报告页）。 */
  report: ScanReport | null
}

/** 批量体检汇总：评分口径只统计已完成仓库。 */
export interface BatchSummary {
  /** 队列仓库数。 */
  total: number
  /** 已完成数。 */
  done: number
  /** 跳过数（取消）。 */
  skipped: number
  /** 失败数。 */
  failed: number
  /** 已完成仓库的平均总分；无有效评分为 null。 */
  avgScore: number | null
  /** 最高分仓库。 */
  best: BatchItem | null
  /** 最低分仓库。 */
  worst: BatchItem | null
  /** 问题总数。 */
  findingCount: number
  /** 严重问题总数。 */
  criticalCount: number
  /** 整批用时（毫秒）；进行中为已耗时。 */
  durationMs: number
}
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
  /** 历史范围受限时的告知文案；全量扫描为 null。 */
  scopeNote: string | null
}

export interface ProgressEvent {
  stage: string
  checkerId?: string
  done: number
  total: number
  /** 0-100 估算进度百分比（后端按阶段统一计算，历史采集阶段按提交比例推进）。 */
  percent?: number
  /** 检查器产出的 finding 数（check 阶段完成事件携带；进入事件为 null/undefined）。 */
  findingCount?: number | null
}

/** 单条自定义规则（字段语义见 docs/custom-rules.md，与后端 CustomRuleDto 一一对应）。 */
export interface CustomRule {
  /** 稳定唯一标识（字母数字与 . _ -）。 */
  id: string
  /** 结论标题。 */
  title: string
  /** 严重程度。 */
  severity: Severity
  /** 所属维度。 */
  category: Category
  /** 匹配用正则（线性时间匹配）。 */
  pattern: string
  /** 路径 glob 限定；null = 不限路径。 */
  pathGlob: string | null
  /** 是否忽略大小写。 */
  ignoreCase: boolean
  /** 可执行的中文改进建议。 */
  suggestion: string
}

/** 扫描配置：用户可调整大文件阈值、历史范围、启用检查器集合与自定义规则。 */
export interface ScanConfig {
  /** 大文件判定阈值（字节）。 */
  bigFileThreshold: number
  /** 僵尸分支「超 N 天未活跃」判定阈值（天）。 */
  zombieBranchDays: number
  /** 启用的检查器 id 列表（空表示全部启用）。 */
  enabledCheckers: string[]
  /** 历史遍历提交数上限（保留最新的若干个）；null = 不限。 */
  maxCommits: number | null
  /** 历史时间窗（天，仅遍历最近 N 天的提交）；null = 不限。 */
  sinceDays: number | null
  /** 自定义规则（空数组 = 不启用 custom-rules 检查器）。 */
  customRules: CustomRule[]
}

/** 仓库规模档位（与后端 ScaleLevel 一致，snake_case）。 */
export type ScaleLevel = 'small' | 'medium' | 'large' | 'huge'

/** 扫描前的规模预估：提交数/文件数为精确值，对象数与耗时为采样外推的粗估。 */
export interface ScaleEstimate {
  commitCount: number
  fileCount: number
  totalBytes: number
  avgObjectsPerCommit: number
  estimatedObjects: number
  estimatedSeconds: number
  level: ScaleLevel
  /** 建议的历史提交数上限；null = 无需限制。 */
  suggestedMaxCommits: number | null
  /** 建议的历史时间窗（天）；null = 无需限制。 */
  suggestedSinceDays: number | null
}

/** 全部内置检查器 id（与后端注册顺序一致）。 */
export const ALL_CHECKER_IDS = [
  'big-files',
  'secrets',
  'conflict-markers',
  'zombie-branches',
  'commit-habits',
  'deps-health',
] as const

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

/** 趋势图中的一次扫描（已按保存时间升序排列）。 */
export interface TrendPoint {
  /** 保存时间（epoch ms）。 */
  savedAt: number
  /** 总分（已覆盖维度平均）；全部维度未覆盖时为 null。 */
  total: number | null
  /** 五维当次评分，键与 radarPoints.key 一致；null = 该维度当次未覆盖。 */
  dims: Record<string, number | null>
}

/** 趋势图仓库选项：历史中出现过的仓库及其扫描次数。 */
export interface TrendRepo {
  /** 仓库路径（分组键）。 */
  repoPath: string
  /** 仓库名（路径末段）。 */
  repoName: string
  /** 该仓库的历史条数。 */
  count: number
  /** 最近一次保存时间（epoch ms），用于倒序排列。 */
  latestAt: number
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