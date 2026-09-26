import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as ipc from '../ipc/commands'
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

  async function startScan(path: string) {
    isScanning.value = true
    isCancelled.value = false
    report.value = null
    progress.value = null
    checkerFeed.value = []
    try {
      await ipc.startScan(path)
    } catch (e) {
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

  function setCurrentView(view: 1 | 2 | 3) {
    currentView.value = view
  }

  function setReport(r: ScanReport) {
    report.value = r
    isScanning.value = false
  }

  function setProgress(p: ProgressEvent) {
    progress.value = p
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
    setFilter,
    toggleFinding,
    resetToConnect,
    setCurrentView,
    setReport,
    setProgress,
    setCheckerFeed,
  }
})