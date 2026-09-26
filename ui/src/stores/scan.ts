import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as ipc from '../ipc/commands'
import { onScanDone, onScanProgress, onScanCancelled, onScanError } from '../ipc/events'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { RepoSnapshot, ScanReport, CheckerFeedRow, ProgressEvent } from '../types/models'

export const useScanStore = defineStore('scan', () => {
  const currentView = ref<1 | 2 | 3>(1)
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
      // scan-done：写入报告并跳转报告页
      onScanDone((r) => {
        if (stale()) return
        unlistenScanEvents()
        report.value = r
        isScanning.value = false
        currentView.value = 3
      }),
      // scan-progress：更新进度
      onScanProgress((p) => {
        if (stale()) return
        progress.value = p
      }),
      // scan-cancelled：用户取消，导航到报告页展示已取消状态
      onScanCancelled((error) => {
        if (stale()) return
        unlistenScanEvents()
        isCancelled.value = true
        isScanning.value = false
        repoError.value = error
        currentView.value = 3
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
      await ipc.startScan(path)
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

  function setCurrentView(view: 1 | 2 | 3) {
    currentView.value = view
  }

  function setCheckerFeed(feed: CheckerFeedRow[]) {
    checkerFeed.value = feed
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
  }
})
