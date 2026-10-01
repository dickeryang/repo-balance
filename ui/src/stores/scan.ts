import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as ipc from '../ipc/commands'
import { onScanDone, onScanProgress, onScanError } from '../ipc/events'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { RepoSnapshot, ScanReport, CheckerFeedRow, ProgressEvent, ScanConfig, HistoryEntry, HistoryCompare, Finding } from '../types/models'

const DEFAULT_CONFIG: ScanConfig = { bigFileThreshold: 1_048_576, enabledCheckers: ['big-files'] }
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

function loadHistory(): HistoryEntry[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY)
    if (raw) return JSON.parse(raw) as HistoryEntry[]
  } catch {
    /* 解析失败回退空列表 */
  }
  return []
}

function persistHistory(entries: HistoryEntry[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(entries))
  } catch {
    /* 忽略持久化失败 */
  }
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
  const history = ref<HistoryEntry[]>(loadHistory())

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
      repoError.value = String(e)
    }
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
        if (!r.cancelled) saveToHistory(r)
        currentView.value = 3
      }),
      // scan-progress：更新进度
      onScanProgress((p) => {
        if (stale()) return
        progress.value = p
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
    checkerFeed.value = []
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

  function setCheckerFeed(feed: CheckerFeedRow[]) {
    checkerFeed.value = feed
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
    filteredFindings,
    totalScore,
    selectDirectory,
    fetchRepoInfo,
    startScan,
    cancelScan,
    exportReport,
    switchRepo,
    setFilter,
    toggleFinding,
    resetToConnect,
    setCurrentView,
    setCheckerFeed,
    setScanConfig,
    deleteHistory,
    clearHistory,
    compareHistory,
  }
})
