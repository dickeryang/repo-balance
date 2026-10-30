import { computed, ref } from 'vue'
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { getVersion } from '@tauri-apps/api/app'

/**
 * 更新流程阶段。UI 只据此渲染状态与按钮，不自己拼状态机。
 *
 * idle → checking → latest
 *                └→ available → downloading → ready
 *                     ↑            └→ error（可重试）
 */
export type UpdatePhase = 'idle' | 'checking' | 'latest' | 'available' | 'downloading' | 'ready' | 'error'

/** 服务端公布的新版本信息（已从 Update 句柄里摘出来，卸载句柄后仍可展示）。 */
export interface UpdatePending {
  version: string
  date: string | null
  notes: string | null
}

/** 可注入依赖，便于单测在进程内跑完整流程（不触达 Tauri 运行时）。 */
export interface UpdaterDeps {
  check: () => Promise<Update | null>
  relaunch: () => Promise<void>
  getVersion: () => Promise<string>
}

/** 「启动时自动检查」偏好的存储键（按机器保存，不写入被检查的仓库）。 */
export const AUTO_CHECK_KEY = 'rb-update-auto-check'

/** 读取自动检查开关；localStorage 不可用或值非法时一律视为关闭。 */
export function loadAutoCheck(): boolean {
  try {
    return localStorage.getItem(AUTO_CHECK_KEY) === '1'
  } catch {
    return false
  }
}

/** 持久化自动检查开关；失败静默（下次启动回到「关闭」是安全侧默认）。 */
export function saveAutoCheck(enabled: boolean): void {
  try {
    localStorage.setItem(AUTO_CHECK_KEY, enabled ? '1' : '0')
  } catch {
    /* 存储不可用时保持本次会话内的设置 */
  }
}

/**
 * 下载进度百分比。
 * 总长度未知（服务端未给 content-length）时返回 null —— 不猜进度，UI 改显已下载量。
 */
export function progressPercent(received: number, total: number | null): number | null {
  if (total === null || !Number.isFinite(total) || total <= 0) return null
  const value = Math.round((received / total) * 100)
  return Math.min(100, Math.max(0, value))
}

/** 字节数的人类可读形式（MiB 为主，<1 MiB 用 KiB）。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 KiB'
  if (bytes >= 1024 * 1024) return `${(bytes / 1_048_576).toFixed(1)} MiB`
  return `${Math.max(1, Math.round(bytes / 1024))} KiB`
}

/**
 * 应用更新（唯一的联网功能，且只在用户显式触发时发起请求）。
 *
 * 设计取舍：不做后台静默轮询——默认不联网是产品的核心承诺，
 * 「启动时自动检查」是显式 opt-in 的开关，关闭时应用进程不发起任何出站请求。
 */
export function useUpdater(overrides: Partial<UpdaterDeps> = {}) {
  const api: UpdaterDeps = {
    check: overrides.check ?? (() => check({ timeout: 20_000 })),
    relaunch: overrides.relaunch ?? relaunch,
    getVersion: overrides.getVersion ?? getVersion,
  }

  const phase = ref<UpdatePhase>('idle')
  const currentVersion = ref('')
  const pending = ref<UpdatePending | null>(null)
  const received = ref(0)
  const total = ref<number | null>(null)
  const error = ref('')
  const autoCheck = ref(loadAutoCheck())

  /** Rust 侧资源句柄；不安装时必须 close，否则每次检查都会留下一个未释放的 Update。 */
  let handle: Update | null = null

  const busy = computed(() => phase.value === 'checking' || phase.value === 'downloading')
  const percent = computed(() => progressPercent(received.value, total.value))

  /** 摘走当前句柄（同步置空，后续 close 失败也不会留下悬空引用）。 */
  function takeHandle(): Update | null {
    const current = handle
    handle = null
    return current
  }

  async function closeQuietly(target: Update | null) {
    if (!target) return
    try {
      await target.close()
    } catch {
      /* 句柄已释放或运行时不可用：忽略 */
    }
  }

  async function closeHandle() {
    await closeQuietly(takeHandle())
  }

  /** 读取当前版本号；失败留空由 UI 显示占位符。 */
  async function loadVersion() {
    try {
      currentVersion.value = await api.getVersion()
    } catch {
      currentVersion.value = ''
    }
  }

  function setAutoCheck(enabled: boolean) {
    autoCheck.value = enabled
    saveAutoCheck(enabled)
  }

  /** 查询更新；重复点击或查询中忽略。 */
  async function checkNow() {
    if (busy.value) return
    // 先同步置为 checking：连点两次时第二次必须在任何 await 之前就被挡掉，
    // 否则会并发发起两次查询（句柄与状态互相覆盖）。
    phase.value = 'checking'
    pending.value = null
    received.value = 0
    total.value = null
    error.value = ''
    await closeQuietly(takeHandle())
    try {
      const found = await api.check()
      if (!found) {
        phase.value = 'latest'
        return
      }
      handle = found
      pending.value = {
        version: found.version,
        date: found.date ?? null,
        notes: found.body?.trim() ? found.body.trim() : null,
      }
      phase.value = 'available'
    } catch (e) {
      phase.value = 'error'
      error.value = String(e)
    }
  }

  /** 下载并安装；失败保留句柄与版本信息，允许原地重试。 */
  async function installNow() {
    if (!handle || phase.value === 'downloading') return
    received.value = 0
    total.value = null
    error.value = ''
    phase.value = 'downloading'
    try {
      await handle.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === 'Started') {
          total.value = event.data.contentLength ?? null
        } else if (event.event === 'Progress') {
          received.value += event.data.chunkLength
        }
      })
      handle = null
      phase.value = 'ready'
    } catch (e) {
      phase.value = 'error'
      error.value = String(e)
    }
  }

  /** 安装完成后重启自身（Windows 上被动安装可能已自重启，此时调用无效但无害）。 */
  async function restartNow() {
    try {
      await api.relaunch()
    } catch (e) {
      phase.value = 'error'
      error.value = String(e)
    }
  }

  /** 卸载时释放句柄，避免 Rust 侧资源泄漏。 */
  function dispose() {
    void closeHandle()
  }

  return {
    phase,
    currentVersion,
    pending,
    received,
    total,
    percent,
    error,
    autoCheck,
    busy,
    loadVersion,
    setAutoCheck,
    checkNow,
    installNow,
    restartNow,
    dispose,
  }
}
