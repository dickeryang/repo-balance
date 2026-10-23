/**
 * store 单元测试：配置持久化、历史合并、评分口径、趋势聚合与批量调度。
 *
 * 覆盖重点：
 * - 纯函数（配置解析 / 历史合并 / 评分）的边界与回退；
 * - 历史 20 条上限、旧键迁移；
 * - 批量调度状态机：失败不中断、取消后剩余跳过、中止不留「进行中」；
 * - 汇总只统计已完成仓库（未覆盖维度不参与平均）。
 *
 * IPC 与 Tauri 事件全部 mock：`startScan` 的 mock 通过 `primeRounds` 预置的
 * 每轮结果同步回推事件，等价于后端即时完成，从而在进程内跑完整个批量循环。
 */
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Finding, HistoryEntry, ScanReport, ScanConfig, Severity } from '../types/models'
import {
  HISTORY_MAX,
  loadConfig,
  mergeHistory,
  parseHistory,
  repoNameOf,
  totalOrNullOf,
  totalScoreOf,
  useScanStore,
} from './scan'

vi.mock('../ipc/commands', () => ({
  startScan: vi.fn(),
  startRemoteScan: vi.fn(),
  cancelScan: vi.fn(async () => undefined),
  repoInfo: vi.fn(),
  scanEstimate: vi.fn(),
  selectDirectory: vi.fn(),
  selectDirectories: vi.fn(async () => []),
  loadScanHistory: vi.fn(async () => null),
  saveScanHistory: vi.fn(async () => undefined),
  importScanHistory: vi.fn(async () => null),
  exportScanHistory: vi.fn(async () => null),
  importCustomRules: vi.fn(async () => null),
  exportCustomRules: vi.fn(async () => null),
  exportReport: vi.fn(async () => null),
}))

vi.mock('../ipc/events', () => {
  const handlers: {
    done: ((report: ScanReport) => void) | null
    progress: ((progress: unknown) => void) | null
    error: ((message: string) => void) | null
  } = { done: null, progress: null, error: null }
  const unlisten = async () => undefined
  return {
    __handlers: handlers,
    onScanDone: (cb: (report: ScanReport) => void) => {
      handlers.done = cb
      return Promise.resolve(unlisten)
    },
    onScanProgress: (cb: (progress: unknown) => void) => {
      handlers.progress = cb
      return Promise.resolve(unlisten)
    },
    onScanCancelled: () => Promise.resolve(unlisten),
    onScanError: (cb: (message: string) => void) => {
      handlers.error = cb
      return Promise.resolve(unlisten)
    },
  }
})

import * as ipc from '../ipc/commands'
import * as events from '../ipc/events'

/** 事件 mock 暴露的事件处理器（测试用它模拟后端回推）。 */
const scanHandlers = (
  events as unknown as {
    __handlers: {
      done: ((report: ScanReport) => void) | null
      error: ((message: string) => void) | null
    }
  }
).__handlers

/** 五维键与文案（与后端 radarPoints 顺序一致）。 */
const DIM_KEYS = ['structure', 'history', 'branches', 'deps', 'security'] as const
const DIM_NAMES = ['结构', '历史', '分支', '依赖', '安全']

/** 构造一份扫描报告：默认五维各 100 分、无问题。 */
function makeReport(
  repoPath: string,
  options: {
    scores?: (number | null)[]
    findings?: Finding[]
    cancelled?: boolean
  } = {},
): ScanReport {
  const scores = options.scores ?? [100, 100, 100, 100, 100]
  return {
    repoPath,
    findings: options.findings ?? [],
    scores: {
      structure: scores[0],
      history: scores[1],
      branches: scores[2],
      deps: scores[3],
      security: scores[4],
    },
    radarPoints: DIM_KEYS.map((key, index) => ({
      key,
      name: DIM_NAMES[index],
      score: scores[index],
      findingCount: 0,
    })),
    startedAt: 1_700_000_000,
    durationMs: 12,
    engineVersion: '0.1.0',
    cancelled: options.cancelled ?? false,
    scopeNote: null,
  }
}

/** 构造一条问题。 */
function makeFinding(severity: Severity, id: string): Finding {
  return { id, severity, category: 'structure', title: 't', evidence: 'e', suggestion: 's' }
}

/** 构造一条历史记录。 */
function makeEntry(id: string, savedAt: number, repoPath = `/repos/${id}`): HistoryEntry {
  return {
    id,
    savedAt,
    repoPath,
    repoName: repoNameOf(repoPath),
    totalScore: 100,
    findingCount: 0,
    report: makeReport(repoPath),
  }
}

/** 预置的每一轮扫描结果。 */
type Round =
  | { kind: 'done'; report: ScanReport }
  | { kind: 'error'; error: string }
  /** 挂起：不推送任何事件（用于验证「中止在途批量」不会永久等待）。 */
  | { kind: 'hang' }

let rounds: Round[] = []
let startedPaths: string[] = []

/** 设置 `ipc.startScan` 的每轮结果（按调用顺序消费）。 */
function primeRounds(next: Round[]) {
  rounds = next
}

/** 每轮开始前的钩子（测试可用它模拟用户在批次中途点取消）。 */
let beforeRound: ((path: string) => void) | null = null

function installStartScanMock() {
  (ipc.startScan as Mock).mockImplementation(async (path: string) => {
    startedPaths.push(path)
    beforeRound?.(path)
    const round = rounds.shift()
    if (!round) throw new Error(`未预置 ${path} 的扫描结果`)
    if (round.kind === 'error') {
      scanHandlers.error?.(round.error)
      return
    }
    if (round.kind === 'done') {
      scanHandlers.done?.(round.report)
    }
    // hang：不推送事件，等待 resetToConnect / 取消把它收尾
  })
}

beforeEach(() => {
  localStorage.clear()
  rounds = []
  startedPaths = []
  beforeRound = null
  scanHandlers.done = null
  scanHandlers.error = null
  vi.clearAllMocks()
  ;(ipc.loadScanHistory as Mock).mockResolvedValue(null)
  ;(ipc.saveScanHistory as Mock).mockResolvedValue(undefined)
  installStartScanMock()
  setActivePinia(createPinia())
})

/** 建一个已载入历史（空）的 store。 */
async function freshStore() {
  const store = useScanStore()
  await store.initHistory()
  return store
}

describe('配置', () => {
  it('setScanConfig_持久化到localStorage并可读回', () => {
    const store = useScanStore()
    const next: ScanConfig = { ...store.scanConfig, bigFileThreshold: 2048 }
    store.setScanConfig(next)

    const raw = localStorage.getItem('rb-scan-config')
    expect(raw).not.toBeNull()
    expect(JSON.parse(raw as string).bigFileThreshold).toBe(2048)
    expect(loadConfig().bigFileThreshold).toBe(2048)
  })

  it('loadConfig_损坏JSON回退默认值', () => {
    localStorage.setItem('rb-scan-config', '{not json')
    const cfg = loadConfig()
    expect(cfg.bigFileThreshold).toBe(1_048_576)
    expect(cfg.customRules).toEqual([])
  })

  it('loadConfig_旧配置缺字段时按默认值补齐', () => {
    localStorage.setItem('rb-scan-config', JSON.stringify({ bigFileThreshold: 8 }))
    const cfg = loadConfig()
    expect(cfg.bigFileThreshold).toBe(8)
    expect(cfg.zombieBranchDays).toBe(90)
    expect(cfg.customRules).toEqual([])
    expect(cfg.enabledCheckers.length).toBeGreaterThan(0)
  })
})

describe('视图导航', () => {
  it('setCurrentView_可在设置页与接入页之间切换', () => {
    const store = useScanStore()
    store.setCurrentView(6)
    expect(store.currentView).toBe(6)
    store.setCurrentView(1)
    expect(store.currentView).toBe(1)
  })
})

describe('评分口径', () => {
  it('totalScoreOf_全维度N_A时返回0', () => {
    const report = makeReport('/r/a', { scores: [null, null, null, null, null] })
    expect(totalScoreOf(report)).toBe(0)
  })

  it('totalOrNullOf_全维度N_A返回null且未覆盖维度不参与平均', () => {
    expect(totalOrNullOf(makeReport('/r/a', { scores: [null, null, null, null, null] }))).toBeNull()
    expect(totalOrNullOf(makeReport('/r/a', { scores: [90, null, 80, null, null] }))).toBe(85)
  })
})

describe('历史纯函数', () => {
  it('parseHistory_损坏或非数组内容回退空列表', () => {
    expect(parseHistory(null)).toEqual([])
    expect(parseHistory('oops')).toEqual([])
    expect(parseHistory('{"a":1}')).toEqual([])
  })

  it('mergeHistory_同id保留现有条目并按保存时间倒序截断', () => {
    const current = [makeEntry('b', 200), makeEntry('a', 100)]
    const incoming = [makeEntry('a', 999), makeEntry('c', 300)]

    const merged = mergeHistory(current, incoming)
    expect(merged.map((e) => e.id)).toEqual(['c', 'b', 'a'])
    expect(merged.find((e) => e.id === 'a')?.savedAt).toBe(100)

    const many = Array.from({ length: HISTORY_MAX + 10 }, (_, i) => makeEntry(`x${i}`, i))
    expect(mergeHistory(many, []).length).toBe(HISTORY_MAX)
  })

  it('repoNameOf_兼容尾部分隔符与无分隔符路径', () => {
    expect(repoNameOf('/a/b/c/')).toBe('c')
    expect(repoNameOf('relative')).toBe('relative')
  })
})

describe('历史载入与写入', () => {
  it('initHistory_载入文件历史并一次性迁移旧localStorage键', async () => {
    localStorage.setItem('rb-scan-history', JSON.stringify([makeEntry('old', 10)]))
    ;(ipc.loadScanHistory as Mock).mockResolvedValue(
      JSON.stringify([makeEntry('file', 20), makeEntry('old', 10)]),
    )

    const store = await freshStore()

    expect(store.history.map((e) => e.id)).toEqual(['file', 'old'])
    expect(localStorage.getItem('rb-scan-history')).toBeNull()
    expect(ipc.saveScanHistory).toHaveBeenCalled()
  })

  it('initHistory_文件读取失败时不清空旧键也不写盘', async () => {
    localStorage.setItem('rb-scan-history', JSON.stringify([makeEntry('old', 10)]))
    ;(ipc.loadScanHistory as Mock).mockRejectedValue(new Error('读取失败'))
    // store 会记录失败原因与「跳过写入」告警，测试中静音以保持输出干净
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined)

    const store = await freshStore()

    errorSpy.mockRestore()
    warnSpy.mockRestore()
    expect(store.history.map((e) => e.id)).toEqual(['old'])
    // 历史未成功载入时写入会被丢弃，避免覆盖磁盘上的既有数据
    expect(ipc.saveScanHistory).not.toHaveBeenCalled()
  })

  it('deleteHistory_与clearHistory_均写盘', async () => {
    const store = await freshStore()
    store.history = [makeEntry('a', 1), makeEntry('b', 2)]

    store.deleteHistory('a')
    expect(store.history.map((e) => e.id)).toEqual(['b'])
    expect(ipc.saveScanHistory).toHaveBeenCalledTimes(1)

    store.clearHistory()
    expect(store.history).toEqual([])
    expect(ipc.saveScanHistory).toHaveBeenCalledTimes(2)
  })
})

describe('趋势聚合', () => {
  it('trendRepos_按仓库分组并倒序排列', async () => {
    const store = await freshStore()
    store.history = [
      makeEntry('a1', 100, '/repos/alpha'),
      makeEntry('b1', 300, '/repos/beta'),
      makeEntry('a2', 200, '/repos/alpha'),
    ]

    expect(store.trendRepos.map((r) => [r.repoName, r.count])).toEqual([
      ['beta', 1],
      ['alpha', 2],
    ])
  })

  it('trendSeries_按时间升序且N_A维度保持null', async () => {
    const store = await freshStore()
    store.history = [
      {
        ...makeEntry('a2', 200, '/repos/alpha'),
        report: makeReport('/repos/alpha', { scores: [80, 80, null, null, null] }),
      },
      {
        ...makeEntry('a1', 100, '/repos/alpha'),
        report: makeReport('/repos/alpha', { scores: [90, 90, 90, 90, 90] }),
      },
    ]

    const series = store.trendSeries('/repos/alpha')
    expect(series.map((p) => p.savedAt)).toEqual([100, 200])
    expect(series[0].total).toBe(90)
    expect(series[1].total).toBe(80)
    expect(series[1].dims.deps).toBeNull()
  })
})

describe('批量体检', () => {
  it('单仓失败不中断整批_汇总只统计已完成仓库', async () => {
    primeRounds([
      {
        kind: 'done',
        report: makeReport('/repos/a', {
          scores: [90, 90, 90, 90, 90],
          findings: [makeFinding('warning', 'w1')],
        }),
      },
      { kind: 'error', error: '不是 git 仓库' },
      {
        kind: 'done',
        report: makeReport('/repos/c', {
          scores: [70, 70, 70, 70, 70],
          findings: [makeFinding('critical', 'c1'), makeFinding('warning', 'w2')],
        }),
      },
    ])
    const store = await freshStore()

    await store.startBatch(['/repos/a', '/repos/b', '/repos/c'])

    expect(startedPaths).toEqual(['/repos/a', '/repos/b', '/repos/c'])
    expect(store.batchItems.map((i) => i.status)).toEqual(['done', 'failed', 'done'])
    expect(store.batchItems[1].error).toContain('不是 git 仓库')

    const summary = store.batchSummary
    expect(summary.total).toBe(3)
    expect(summary.done).toBe(2)
    expect(summary.failed).toBe(1)
    expect(summary.avgScore).toBe(80)
    expect(summary.findingCount).toBe(3)
    expect(summary.criticalCount).toBe(1)
    expect(summary.best?.name).toBe('a')
    expect(summary.worst?.name).toBe('c')

    // 完成的仓库写入历史，失败的仓库不入历史
    expect(store.history.length).toBe(2)
    expect(store.batchRunning).toBe(false)
    expect(store.currentView).toBe(5)
  })

  it('startBatch_同目录去重_结束后可再次发起并替换队列', async () => {
    primeRounds([
      { kind: 'done', report: makeReport('/repos/a') },
      { kind: 'done', report: makeReport('/repos/z') },
    ])
    const store = await freshStore()

    await store.startBatch(['/repos/a', '/repos/a'])
    expect(startedPaths).toEqual(['/repos/a'])
    expect(store.batchItems.length).toBe(1)

    // 整批结束后允许再次发起（例如补扫新仓库），队列被整体替换
    await store.startBatch(['/repos/z'])
    expect(startedPaths).toEqual(['/repos/a', '/repos/z'])
    expect(store.batchItems.map((i) => i.name)).toEqual(['z'])
  })

  it('取消批量_当前仓库标记跳过且剩余仓库不再启动', async () => {
    primeRounds([
      { kind: 'done', report: makeReport('/repos/a') },
      {
        kind: 'done',
        report: makeReport('/repos/b', { cancelled: true }),
      },
    ])
    const store = await freshStore()
    beforeRound = (path) => {
      if (path === '/repos/b') void store.cancelBatch()
    }

    await store.startBatch(['/repos/a', '/repos/b', '/repos/c'])

    expect(startedPaths).toEqual(['/repos/a', '/repos/b'])
    expect(store.batchItems.map((i) => i.status)).toEqual(['done', 'skipped', 'skipped'])
    expect(store.batchRunning).toBe(false)
    expect(store.history.length).toBe(1)
  })

  it('resetToConnect_中止在途批量不会永久卡在进行中', async () => {
    primeRounds([{ kind: 'hang' }])
    const store = await freshStore()

    const running = store.startBatch(['/repos/a', '/repos/b'])
    // 让同步部分执行到「等待本轮事件」的位置
    await Promise.resolve()
    expect(store.batchRunning).toBe(true)

    store.resetToConnect()
    await running

    expect(store.currentView).toBe(1)
    expect(store.batchRunning).toBe(false)
    expect(store.batchItems[0].status).toBe('failed')
    expect(store.batchItems[0].error).toContain('中止')
    expect(store.batchItems[1].status).toBe('skipped')
  })

  it('openBatchReport_加载报告并跳转报告页_快照失败不影响查看', async () => {
    primeRounds([
      {
        kind: 'done',
        report: makeReport('/repos/a', { findings: [makeFinding('critical', 'c1')] }),
      },
    ])
    ;(ipc.repoInfo as Mock).mockRejectedValue('仓库不可达')
    const store = await freshStore()
    await store.startBatch(['/repos/a'])

    await store.openBatchReport(0)

    expect(store.currentView).toBe(3)
    expect(store.report?.repoPath).toBe('/repos/a')
    expect(store.repoInfo).toBeNull()
  })

  it('批量写入历史同样遵守20条上限', async () => {
    const count = HISTORY_MAX + 5
    primeRounds(
      Array.from({ length: count }, (_, i) => ({
        kind: 'done' as const,
        report: makeReport(`/repos/r${i}`),
      })),
    )
    const store = await freshStore()

    await store.startBatch(Array.from({ length: count }, (_, i) => `/repos/r${i}`))

    expect(store.batchItems.every((i) => i.status === 'done')).toBe(true)
    expect(store.history.length).toBe(HISTORY_MAX)
  })

  it('批量写入历史_串行写盘且每份快照都完整', async () => {
    primeRounds([
      { kind: 'done', report: makeReport('/repos/a') },
      { kind: 'done', report: makeReport('/repos/b') },
    ])
    const store = await freshStore()

    await store.startBatch(['/repos/a', '/repos/b'])

    const calls = (ipc.saveScanHistory as Mock).mock.calls
    expect(calls.length).toBe(2)
    // 每次写盘的内容都是「调用时刻的完整历史」：串行 await 保证不回退
    expect(parseHistory(calls[0][0] as string).length).toBe(1)
    expect(parseHistory(calls[1][0] as string).length).toBe(2)
  })

  it('clearBatch_清空队列并复位计时', async () => {
    primeRounds([{ kind: 'done', report: makeReport('/repos/a') }])
    const store = await freshStore()
    await store.startBatch(['/repos/a'])

    store.clearBatch()

    expect(store.batchItems).toEqual([])
    expect(store.batchStartedAt).toBeNull()
    expect(store.batchFinishedAt).toBeNull()
    expect(store.batchSummary.total).toBe(0)
    expect(store.batchSummary.avgScore).toBeNull()
  })
})

// 远程体检：URL 来源没有本地路径，接入页/进度页/失败路径都要按 URL 走，
// 且失败时错误必须可见（早年错误卡片只在 selectedPath 非空时渲染，
// 远程失败会被静默吞掉——用户只看到「回到接入页」）。
describe('远程体检', () => {
  it('startRemoteScan_记住URL并清掉本地仓库目标', async () => {
    const store = await freshStore()
    store.selectedPath = '/tmp/local-repo'

    await store.startRemoteScan('https://example.com/u/r.git')

    expect(store.remoteUrl).toBe('https://example.com/u/r.git')
    expect(store.selectedPath).toBeNull()
    // 远程体检同样进入扫描页（不能因为没有本地路径就停在接入页）
    expect(store.currentView).toBe(2)
  })

  it('远程体检_完成后进入报告页且报告路径为URL', async () => {
    const store = await freshStore()
    await store.startRemoteScan('https://example.com/u/r.git')

    scanHandlers.done?.(makeReport('https://example.com/u/r.git'))

    expect(store.currentView).toBe(3)
    expect(store.isScanning).toBe(false)
    expect(store.report?.repoPath).toBe('https://example.com/u/r.git')
  })

  it('远程体检_失败时停在接入页并保留可见错误', async () => {
    const store = await freshStore()
    ;(ipc.startRemoteScan as Mock).mockRejectedValueOnce(new Error('克隆失败'))

    await store.startRemoteScan('https://example.com/u/r.git')

    // 失败应停在接入页并展示错误，而不是留在扫描页空转
    expect(store.currentView).toBe(1)
    expect(store.repoError).toContain('克隆失败')
    expect(store.isScanning).toBe(false)
    // 关键回归：远程来源没有 selectedPath，接入页不能再以它为条件隐藏错误卡片。
    expect(store.selectedPath).toBeNull()
  })
})
