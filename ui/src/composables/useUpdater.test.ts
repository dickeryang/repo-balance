/**
 * 更新流程状态机单测：检查 / 发现新版本 / 下载进度 / 安装失败重试 / 自动检查偏好。
 *
 * 三个 Tauri 模块全部 mock，测试在 Node 进程内跑完整流程，不触达真实运行时与网络。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { DownloadEvent, Update } from '@tauri-apps/plugin-updater'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { getVersion } from '@tauri-apps/api/app'
import {
  AUTO_CHECK_KEY,
  formatBytes,
  loadAutoCheck,
  progressPercent,
  saveAutoCheck,
  useUpdater,
} from './useUpdater'

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn(async () => undefined) }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn(async () => '0.1.0') }))

const checkMock = check as unknown as ReturnType<typeof vi.fn>
const relaunchMock = relaunch as unknown as ReturnType<typeof vi.fn>
const getVersionMock = getVersion as unknown as ReturnType<typeof vi.fn>

/** 伪造一个 Update 句柄：下载时按预置事件序列回推。 */
function fakeUpdate(options: { events?: DownloadEvent[]; failDownload?: boolean } = {}) {
  const events = options.events ?? [{ event: 'Finished' } as DownloadEvent]
  return {
    version: '0.2.0',
    date: '2026-10-30T08:00:00Z',
    body: '  新增批量体检\n  ',
    close: vi.fn(async () => undefined),
    downloadAndInstall: vi.fn(async (onEvent: (e: DownloadEvent) => void) => {
      if (options.failDownload) throw new Error('磁盘写入失败')
      for (const event of events) onEvent(event)
    }),
  } as unknown as Update
}

beforeEach(() => {
  checkMock.mockReset()
  relaunchMock.mockReset()
  getVersionMock.mockReset()
  getVersionMock.mockResolvedValue('0.1.0')
  localStorage.removeItem(AUTO_CHECK_KEY)
})

describe('纯函数', () => {
  it('progressPercent_总长度未知返回null_否则夹紧到0到100', () => {
    expect(progressPercent(10, null)).toBeNull()
    expect(progressPercent(10, 0)).toBeNull()
    expect(progressPercent(0, 1000)).toBe(0)
    expect(progressPercent(250, 1000)).toBe(25)
    expect(progressPercent(2000, 1000)).toBe(100)
    expect(progressPercent(-5, 1000)).toBe(0)
  })

  it('formatBytes_按MiB与KiB分档且非法值归零', () => {
    expect(formatBytes(0)).toBe('0 KiB')
    expect(formatBytes(-1)).toBe('0 KiB')
    expect(formatBytes(512)).toBe('1 KiB')
    expect(formatBytes(1536)).toBe('2 KiB')
    expect(formatBytes(3 * 1_048_576)).toBe('3.0 MiB')
  })

  it('自动检查偏好_默认关闭_写入后可读回', () => {
    expect(loadAutoCheck()).toBe(false)
    saveAutoCheck(true)
    expect(localStorage.getItem(AUTO_CHECK_KEY)).toBe('1')
    expect(loadAutoCheck()).toBe(true)
    saveAutoCheck(false)
    expect(loadAutoCheck()).toBe(false)
  })
})

describe('检查更新', () => {
  it('checkNow_无新版本时进入latest', async () => {
    checkMock.mockResolvedValue(null)
    const u = useUpdater()
    await u.checkNow()
    expect(u.phase.value).toBe('latest')
    expect(u.pending.value).toBeNull()
  })

  it('checkNow_有新版本时记录版本与去空白后的说明', async () => {
    checkMock.mockResolvedValue(fakeUpdate())
    const u = useUpdater()
    await u.checkNow()
    expect(u.phase.value).toBe('available')
    expect(u.pending.value).toEqual({
      version: '0.2.0',
      date: '2026-10-30T08:00:00Z',
      notes: '新增批量体检',
    })
  })

  it('checkNow_重复检查会释放上一次句柄', async () => {
    const first = fakeUpdate()
    const second = fakeUpdate()
    checkMock.mockResolvedValueOnce(first).mockResolvedValueOnce(second)
    const u = useUpdater()
    await u.checkNow()
    await u.checkNow()
    expect((first as unknown as { close: ReturnType<typeof vi.fn> }).close).toHaveBeenCalledTimes(1)
    expect(u.pending.value?.version).toBe('0.2.0')
  })

  it('checkNow_查询失败时进入error并保留可读原因', async () => {
    checkMock.mockRejectedValue(new Error('network unreachable'))
    const u = useUpdater()
    await u.checkNow()
    expect(u.phase.value).toBe('error')
    expect(u.error.value).toContain('network unreachable')
    expect(u.pending.value).toBeNull()
  })

  it('checkNow_查询进行中重复点击被忽略', async () => {
    let release: (v: Update | null) => void = () => {}
    checkMock.mockImplementation(() => new Promise((resolve) => { release = resolve }))
    const u = useUpdater()
    const first = u.checkNow()
    await u.checkNow()
    expect(checkMock).toHaveBeenCalledTimes(1)
    release(null)
    await first
    expect(u.phase.value).toBe('latest')
  })
})

describe('下载与安装', () => {
  it('installNow_按事件累计进度并在完成后进入ready', async () => {
    const update = fakeUpdate({
      events: [
        { event: 'Started', data: { contentLength: 2000 } },
        { event: 'Progress', data: { chunkLength: 1200 } },
        { event: 'Progress', data: { chunkLength: 800 } },
        { event: 'Finished' },
      ],
    })
    checkMock.mockResolvedValue(update)
    const u = useUpdater()
    await u.checkNow()
    await u.installNow()
    expect(u.phase.value).toBe('ready')
    expect(u.received.value).toBe(2000)
    expect(u.total.value).toBe(2000)
    expect(u.percent.value).toBe(100)
  })

  it('installNow_未拿到contentLength时进度为null但已下载量照常累计', async () => {
    const update = fakeUpdate({
      events: [{ event: 'Progress', data: { chunkLength: 700 } }, { event: 'Finished' }],
    })
    checkMock.mockResolvedValue(update)
    const u = useUpdater()
    await u.checkNow()
    await u.installNow()
    expect(u.received.value).toBe(700)
    expect(u.total.value).toBeNull()
    expect(u.percent.value).toBeNull()
    expect(u.phase.value).toBe('ready')
  })

  it('installNow_下载失败保留版本信息并可原地重试', async () => {
    const update = fakeUpdate({ failDownload: true })
    checkMock.mockResolvedValue(update)
    const u = useUpdater()
    await u.checkNow()
    await u.installNow()
    expect(u.phase.value).toBe('error')
    expect(u.pending.value?.version).toBe('0.2.0')
    expect(u.busy.value).toBe(false)

    await u.installNow()
    expect((update as unknown as { downloadAndInstall: ReturnType<typeof vi.fn> }).downloadAndInstall)
      .toHaveBeenCalledTimes(2)
  })

  it('installNow_未检查出更新时直接返回_不调用接口', async () => {
    const u = useUpdater()
    await u.installNow()
    expect(u.phase.value).toBe('idle')
  })

  it('restartNow_调用失败时进入error', async () => {
    relaunchMock.mockRejectedValue(new Error('relaunch denied'))
    const u = useUpdater()
    await u.restartNow()
    expect(u.phase.value).toBe('error')
    expect(u.error.value).toContain('relaunch denied')
  })

  it('dispose_释放未安装的句柄', async () => {
    const update = fakeUpdate()
    checkMock.mockResolvedValue(update)
    const u = useUpdater()
    await u.checkNow()
    u.dispose()
    await Promise.resolve()
    expect((update as unknown as { close: ReturnType<typeof vi.fn> }).close).toHaveBeenCalledTimes(1)
  })
})
