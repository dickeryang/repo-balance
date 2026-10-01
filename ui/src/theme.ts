import { ref, computed } from 'vue'

export type Theme = 'dark' | 'light'

const STORAGE_KEY = 'rb-theme'

function loadTheme(): Theme {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw === 'dark' || raw === 'light') return raw
  } catch {
    /* localStorage 不可用 */
  }
  // 未手动选过主题时跟随系统
  if (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: light)').matches) {
    return 'light'
  }
  return 'dark'
}

function applyTheme(t: Theme) {
  document.documentElement.classList.toggle('dark', t === 'dark')
  document.documentElement.style.colorScheme = t
}

const theme = ref<Theme>(loadTheme())
applyTheme(theme.value)

// 跟随系统：仅在用户未手动选择过主题时生效
let userPinned = false
try {
  userPinned = localStorage.getItem(STORAGE_KEY) !== null
} catch {
  /* 忽略 */
}
if (typeof window !== 'undefined' && !userPinned) {
  window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener?.('change', (e) => {
    if (userPinned) return
    setTheme(e.matches ? 'dark' : 'light')
  })
}

export function setTheme(t: Theme) {
  theme.value = t
  applyTheme(t)
  try {
    localStorage.setItem(STORAGE_KEY, t)
  } catch {
    /* 忽略持久化失败 */
  }
}

export function toggleTheme() {
  setTheme(theme.value === 'dark' ? 'light' : 'dark')
}

export function useTheme() {
  return {
    theme: computed(() => theme.value),
    setTheme,
    toggleTheme,
  }
}
