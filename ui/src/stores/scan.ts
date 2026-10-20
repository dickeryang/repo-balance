import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as ipc from '../ipc/commands'
import { onScanDone, onScanProgress, onScanError } from '../ipc/events'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { RepoSnapshot, ScanReport, CheckerFeedRow, ProgressEvent, ScanConfig, ScaleEstimate, HistoryEntry, HistoryCompare, Finding, TrendPoint, TrendRepo } from '../types/models'
import { ALL_CHECKER_IDS } from '../types/models'

const DEFAULT_CONFIG: ScanConfig = {
  bigFileThreshold: 1_048_576,
  zombieBranchDays: 90,
  enabledCheckers: [...ALL_CHECKER_IDS],
  maxCommits: null,
  sinceDays: null,
  customRules: [],
}
/** 旧版 localStorage 键：仅用于一次性迁移，读走后即清理。 */
const HISTORY_KEY = 'rb-scan-history'
const HISTORY_MAX = 20

function loadConfig(): ScanConfig {
  try {
    const raw = localStorage.getItem('rb-scan-config')
    if (raw) return { ...DEFAULT_CONFIG, ...JSON.parse(raw) }
  } catch {
    /* localStorage 不可用时回退默认 */
  }
  return { ...DEFAULT_CONFIG }
}

/** 解析历史 JSON；非法内容回退空列表（旧数据损坏不应阻断启动）。 */
function parseHistory(raw: string | null): HistoryEntry[] {
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? (parsed as HistoryEntry[]) : []
  } catch {
    return []
  }
}

/**
 * 合并两组历史：现有条目优先（同 id 不覆盖），按保存时间倒序保留最新 HISTORY_MAX 条。
 * 迁移旧数据与导入外部文件共用同一口径。
 */
function mergeHistory(current: HistoryEntry[], incoming: HistoryEntry[]): HistoryEntry[] {
  const ids = new Set(current.map((e) => e.id))
  return [...current, ...incoming.filter((e) => !ids.has(e.id))]
    .sort((a, b) => b.savedAt - a.savedAt)
    .slice(0, HISTORY_MAX)
}

function repoNameOf(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, '')
  const seg = trimmed.split(/[\\/]/).pop()
  return seg || trimmed
}

function totalScoreOf(report: ScanReport): number {
  const scores = report.radarPoints.map((p) => p.score).filter((s): s is number => s !== null)
  if (scores.length === 0) return 0
  return Math.round(scores.reduce((a, b) => a + b, 0) / scores.length)
}

/**
 * 总分（已覆盖维度平均），全部维度未覆盖时返回 null。
 *
 * 与 totalScoreOf 的差异：趋势图必须区分「0 分」与「无结论」，否则会把
 * N/A 画成断崖下跌（诚信口径：未覆盖维度不参与、不伪造）。
 */
function totalOrNullOf(report: ScanReport): number | null {
  const scores = report.radarPoints.map((p) => p.score).filter((s): s is number => s !== null)
  if (scores.length === 0) return null
  return Math.round(scores.reduce((a, b) => a + b, 0) / scores.length)
}

export const useScanStore = defineStore('scan', () => {
  const currentView = ref<1 | 2 | 3 | 4>(1)
  const selectedPath = ref<string | null>(null)
  const repoInfo = ref<RepoSnapshot | null>(null)
  const repoError = ref<string | null>(null)
  const isScanning = ref(false)
  const progress = ref<ProgressEvent | null>(null)
  const checkerFeed = ref<CheckerFeedRow[]>([])
  const report = ref<ScanReport | null>(null)
  const isCancelled = ref(false)
  const activeFilter = ref('all')
  const expandedFindings = ref<Set<string>>(new Set())
  const scanConfig = ref<ScanConfig>(loadConfig())
  // 历史自文件异步加载（initHistory），初始为空列表。
  const history = ref<HistoryEntry[]>([])
  const scaleEstimate = ref<ScaleEstimate | null>(null)
  /** 历史是否已从应用数据目录载入（未完成时写入会被丢弃，防覆盖旧数据）。 */
  let historyLoaded = false

  // 扫描代际号：每次开启新扫描自增；事件回调按注册时捕获的代际过滤，
  // 迟到的陈旧事件（上一轮扫描的 scan-done 等）直接丢弃。
  let scanToken = 0
  // 当前扫描的事件监听注销器：startScan 时注册，结束/重置时注销。
  let unlisteners: Promise<UnlistenFn>[] = []

  const filteredFindings = computed(() => {
    if (!report.value) return []
    const filter = activeFilter.value
    if (filter === 'all') return report.value.findings
    if (['critical', 'warning', 'info'].includes(filter)) {
      return report.value.findings.filter((f) => f.severity === filter)
    }
    return report.value.findings.filter((f) => f.category === filter)
  })

  const totalScore = computed(() => {
    if (!report.value) return 0
    const scores = report.value.radarPoints
      .map((p) => p.score)
      .filter((s): s is number => s !== null)
    if (scores.length === 0) return 0
    return Math.round(scores.reduce((a, b) => a + b, 0) / scores.length)
  })

  /** 历史中出现过的仓库（按最近一次扫描时间倒序），供趋势图选择。 */
  const trendRepos = computed<TrendRepo[]>(() => {
    const map = new Map<string, TrendRepo>()
    for (const e of history.value) {
      const cur = map.get(e.repoPath)
      if (cur) {
        cur.count += 1
        if (e.savedAt > cur.latestAt) cur.latestAt = e.savedAt
      } else {
        map.set(e.repoPath, {
          repoPath: e.repoPath,
          repoName: e.repoName,
          count: 1,
          latestAt: e.savedAt,
        })
      }
    }
    return [...map.values()].sort((a, b) => b.latestAt - a.latestAt)
  })

  /**
   * 指定仓库的趋势点（按保存时间升序）。
   *
   * 维度评分取自当次报告的 radarPoints：N/A 维度保持 null，趋势图据此断线，
   * 不做插值（插值等于伪造未覆盖维度的结论）。
   */
  function trendSeries(repoPath: string): TrendPoint[] {
    return history.value
      .filter((e) => e.repoPath === repoPath)
      .map((e) => ({
        savedAt: e.savedAt,
        total: totalOrNullOf(e.report),
        dims: Object.fromEntries(e.report.radarPoints.map((p) => [p.key, p.score])),
      }))
      .sort((a, b) => a.savedAt - b.savedAt)
  }

  async function selectDirectory() {
    const path = await ipc.selectDirectory()
    if (path) {
      selectedPath.value = path
      repoError.value = null
      await fetchRepoInfo(path)
    }
  }

  async function fetchRepoInfo(path: string) {
    try {
      repoInfo.value = await ipc.repoInfo(path)
      repoError.value = null
    } catch (e) {
      repoInfo.value = null
      scaleEstimate.value = null
      repoError.value = String(e)
      return
    }
    // 规模预估失败不影响扫描（只是少一条提示），静默降级。
    try {
      scaleEstimate.value = await ipc.scanEstimate(path)
    } catch {
      scaleEstimate.value = null
    }
  }

  /**
   * 启动时载入扫描历史：优先应用数据目录文件；
   * 首次运行把旧版 localStorage 数据一次性迁入文件后清理旧键。
   */
  async function initHistory() {
    let entries: HistoryEntry[] = []
    let loaded = false
    try {
      entries = parseHistory(await ipc.loadScanHistory())
      loaded = true
    } catch (e) {
      // 载入失败时文件内容未知，标记为未载入以暂停写入，避免覆盖已有历史。
      console.error('读取扫描历史失败:', e)
    }

    let legacy: HistoryEntry[] = []
    try {
      legacy = parseHistory(localStorage.getItem(HISTORY_KEY))
    } catch {
      legacy = []
    }
    if (legacy.length > 0) {
      // 合并去重后写回文件，再移除旧键（迁移只做一次）。
      entries = mergeHistory(entries, legacy)
      try {
        localStorage.removeItem(HISTORY_KEY)
      } catch {
        /* 忽略清理失败 */
      }
    }

    history.value = entries
    historyLoaded = loaded
    if (legacy.length > 0) await persistHistory(entries)
  }

  /**
   * 导入外部历史文件：与现有历史按 id 合并去重，保留最新的 HISTORY_MAX 条。
   * 返回新增条数（去重后真正并入的条数）；返回 -1 表示用户取消，由调用方决定提示。
   */
  async function importHistory(): Promise<number> {
    const raw = await ipc.importScanHistory()
    if (raw === null) return -1
    const incoming = parseHistory(raw)
    const existingIds = new Set(history.value.map((e) => e.id))
    const added = incoming.filter((e) => !existingIds.has(e.id)).length
    const merged = mergeHistory(history.value, incoming)
    history.value = merged
    await persistHistory(merged)
    return added
  }

  /** 导出全部历史为 JSON 文件；返回实际保存路径（用户取消时为 null）。 */
  async function exportHistory(): Promise<string | null> {
    return ipc.exportScanHistory(JSON.stringify(history.value, null, 2))
  }

  /** 把历史写入应用数据目录文件（未载入完成时跳过，避免覆盖已有数据）。 */
  async function persistHistory(entries: HistoryEntry[]) {
    if (!historyLoaded) {
      console.warn('扫描历史尚未载入完成，本次跳过写入')
      return
    }
    try {
      await ipc.saveScanHistory(JSON.stringify(entries))
    } catch (e) {
      console.error('保存扫描历史失败:', e)
    }
  }

  /** 应用规模预估给出的建议限制（大仓库一键收敛历史范围）。 */
  function applySuggestedLimits() {
    const est = scaleEstimate.value
    if (!est) return
    setScanConfig({
      ...scanConfig.value,
      maxCommits: est.suggestedMaxCommits,
      sinceDays: est.suggestedSinceDays,
    })
  }

  /** 注销当前扫描的事件监听。 */
  function unlistenScanEvents() {
    unlisteners.forEach((p) => p.then((fn) => fn()).catch(() => {}))
    unlisteners = []
  }

  /** 注册本轮扫描的事件监听，回调按代际号过滤陈旧事件。 */
  function listenScanEvents(token: number) {
    unlistenScanEvents()
    const stale = () => token !== scanToken
    unlisteners = [
      // scan-done：写入报告并跳转报告页；仅完成的扫描保存历史。
      // 取消的扫描后端会推送 cancelled=true 的部分报告，同样经此入口渲染，但不入历史。
      onScanDone((r) => {
        if (stale()) return
        unlistenScanEvents()
        report.value = r
        isCancelled.value = r.cancelled
        isScanning.value = false
        // 收尾未收到完成事件的检查器行（如取消时中断在 check 阶段）。
        checkerFeed.value = checkerFeed.value.map((row) =>
          row.status === 'done' ? row : { ...row, status: 'done' },
        )
        if (!r.cancelled) saveToHistory(r)
        currentView.value = 3
      }),
      // scan-progress：更新进度；check 阶段按 checkerId 维护检查器 feed
      onScanProgress((p) => {
        if (stale()) return
        progress.value = p
        if (p.stage === 'check' && p.checkerId) {
          const rows = [...checkerFeed.value]
          let row = rows.find((r) => r.checkerId === p.checkerId)
          if (!row) {
            row = { checkerId: p.checkerId, status: 'pending', findingCount: 0 }
            rows.push(row)
          }
          // 完成事件携带 findingCount（number）；进入事件为 null/undefined。
          if (typeof p.findingCount === 'number') {
            row.status = 'done'
            row.findingCount = p.findingCount
          } else {
            row.status = 'running'
          }
          checkerFeed.value = rows
        }
      }),
      // scan-error：扫描失败，回接入页并展示错误
      onScanError((error) => {
        if (stale()) return
        unlistenScanEvents()
        isScanning.value = false
        repoError.value = error
        currentView.value = 1
      }),
    ]
  }

  async function startScan(path: string) {
    isScanning.value = true
    isCancelled.value = false
    report.value = null
    progress.value = null
    // 初始化检查器 feed：按启用列表（空 = 全部）预置待运行行，
    // 扫描开始后由 check 阶段进度事件驱动 running → done。
    const ids =
      scanConfig.value.enabledCheckers.length > 0
        ? scanConfig.value.enabledCheckers
        : [...ALL_CHECKER_IDS]
    checkerFeed.value = ids.map((id) => ({ checkerId: id, status: 'pending', findingCount: 0 }))
    const token = ++scanToken
    listenScanEvents(token)
    try {
      await ipc.startScan(path, scanConfig.value)
    } catch (e) {
      if (token !== scanToken) return
      unlistenScanEvents()
      isScanning.value = false
      repoError.value = String(e)
      currentView.value = 1
    }
  }

  /** 远程仓库体检：URL 浅克隆到临时目录后扫描（与本地扫描共用事件流）。 */
  async function startRemoteScan(url: string) {
    isScanning.value = true
    isCancelled.value = false
    report.value = null
    progress.value = null
    const ids =
      scanConfig.value.enabledCheckers.length > 0
        ? scanConfig.value.enabledCheckers
        : [...ALL_CHECKER_IDS]
    checkerFeed.value = ids.map((id) => ({ checkerId: id, status: 'pending', findingCount: 0 }))
    const token = ++scanToken
    listenScanEvents(token)
    setCurrentView(2)
    try {
      await ipc.startRemoteScan(url, 1, scanConfig.value)
    } catch (e) {
      if (token !== scanToken) return
      unlistenScanEvents()
      isScanning.value = false
      repoError.value = String(e)
      currentView.value = 1
    }
  }

  async function cancelScan() {
    try {
      await ipc.cancelScan()
      isCancelled.value = true
    } catch (e) {
      console.error('取消扫描失败:', e)
      isCancelled.value = false
    }
  }

  async function exportReport(format: string): Promise<string | null> {
    if (!report.value) return null
    try {
      return await ipc.exportReport(format, report.value)
    } catch (e) {
      console.error('导出失败:', e)
      return null
    }
  }

  function setFilter(filter: string) {
    activeFilter.value = filter
  }

  function toggleFinding(id: string) {
    const next = new Set(expandedFindings.value)
    if (next.has(id)) {
      next.delete(id)
    } else {
      next.add(id)
    }
    expandedFindings.value = next
  }

  function resetToConnect() {
    scanToken++ // 使任何在途扫描的迟到事件全部失效
    unlistenScanEvents()
    currentView.value = 1
    selectedPath.value = null
    repoInfo.value = null
    repoError.value = null
    isScanning.value = false
    progress.value = null
    checkerFeed.value = []
    report.value = null
    isCancelled.value = false
    activeFilter.value = 'all'
    expandedFindings.value = new Set()
    scaleEstimate.value = null
  }

  /** 更换仓库：取消在途扫描、清空全部状态并回到接入页。 */
  async function switchRepo() {
    if (isScanning.value) {
      await cancelScan()
      isScanning.value = false
    }
    resetToConnect()
  }

  function setCurrentView(view: 1 | 2 | 3 | 4) {
    currentView.value = view
  }

  /** 更新扫描配置并持久化到 localStorage。 */
  function setScanConfig(cfg: ScanConfig) {
    scanConfig.value = cfg
    try {
      localStorage.setItem('rb-scan-config', JSON.stringify(cfg))
    } catch {
      /* 忽略持久化失败 */
    }
  }

  /** 将完成的扫描报告保存到历史列表（最多保留 HISTORY_MAX 条）。 */
  function saveToHistory(r: ScanReport) {
    const entry: HistoryEntry = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      savedAt: Date.now(),
      repoPath: r.repoPath,
      repoName: repoNameOf(r.repoPath),
      totalScore: totalScoreOf(r),
      findingCount: r.findings.length,
      report: r,
    }
    const next = [entry, ...history.value].slice(0, HISTORY_MAX)
    history.value = next
    persistHistory(next)
  }

  /** 删除指定历史条目。 */
  function deleteHistory(id: string) {
    const next = history.value.filter((e) => e.id !== id)
    history.value = next
    persistHistory(next)
  }

  /** 清空全部历史。 */
  function clearHistory() {
    history.value = []
    persistHistory([])
  }

  /** 对比两条历史：以 baseline 为基准、target 为对照，计算新增/消除/不变发现项。 */
  function compareHistory(idA: string, idB: string): HistoryCompare | null {
    const baseline = history.value.find((e) => e.id === idA)
    const target = history.value.find((e) => e.id === idB)
    if (!baseline || !target) return null
    const baselineIds = new Set(baseline.report.findings.map((f) => f.id))
    const targetIds = new Set(target.report.findings.map((f) => f.id))
    const addedFindings: Finding[] = []
    const commonFindings: Finding[] = []
    for (const f of target.report.findings) {
      if (baselineIds.has(f.id)) commonFindings.push(f)
      else addedFindings.push(f)
    }
    const resolvedFindings = baseline.report.findings.filter((f) => !targetIds.has(f.id))
    return {
      baseline,
      target,
      scoreDelta: target.totalScore - baseline.totalScore,
      findingCountDelta: target.findingCount - baseline.findingCount,
      addedFindings,
      resolvedFindings,
      commonFindings,
    }
  }

  return {
    currentView,
    selectedPath,
    repoInfo,
    repoError,
    isScanning,
    progress,
    checkerFeed,
    report,
    isCancelled,
    activeFilter,
    expandedFindings,
    scanConfig,
    history,
    scaleEstimate,
    filteredFindings,
    totalScore,
    trendRepos,
    trendSeries,
    selectDirectory,
    fetchRepoInfo,
    initHistory,
    applySuggestedLimits,
    importHistory,
    exportHistory,
    startScan,
    startRemoteScan,
    cancelScan,
    exportReport,
    switchRepo,
    setFilter,
    toggleFinding,
    resetToConnect,
    setCurrentView,
    setScanConfig,
    deleteHistory,
    clearHistory,
    compareHistory,
  }
})
