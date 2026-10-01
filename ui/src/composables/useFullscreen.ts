import { ref, onBeforeUnmount } from 'vue'

/**
 * 元素全屏切换（基于原生 Fullscreen API）。
 * 返回目标 ref、是否处于全屏状态以及切换函数。
 */
export function useFullscreen() {
  const el = ref<HTMLElement | null>(null)
  const isFullscreen = ref(false)

  function onChange() {
    isFullscreen.value = document.fullscreenElement === el.value
  }

  document.addEventListener('fullscreenchange', onChange)
  onBeforeUnmount(() => {
    document.removeEventListener('fullscreenchange', onChange)
    // 组件卸载时若仍处于全屏，退出以避免残留遮罩
    if (document.fullscreenElement === el.value) {
      document.exitFullscreen().catch(() => {})
    }
  })

  async function toggle() {
    if (!el.value) return
    try {
      if (document.fullscreenElement === el.value) {
        await document.exitFullscreen()
      } else {
        await el.value.requestFullscreen()
      }
    } catch {
      /* 用户拒绝或环境不支持时静默失败 */
    }
  }

  return { el, isFullscreen, toggle }
}
