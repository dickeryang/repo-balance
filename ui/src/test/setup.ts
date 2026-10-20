/**
 * 测试环境准备：Node 下没有 `localStorage`，这里提供等价的内存实现。
 *
 * store 用 localStorage 持久化扫描配置，并在启动时一次性迁移旧版历史键；
 * 真实 WebView 一定具备该 API，测试里用内存实现即可覆盖这些逻辑，
 * 无需为此引入 jsdom（依赖更小、启动更快）。
 */
class MemoryStorage {
  private readonly store = new Map<string, string>()

  get length(): number {
    return this.store.size
  }

  clear(): void {
    this.store.clear()
  }

  getItem(key: string): string | null {
    const value = this.store.get(key)
    return value === undefined ? null : value
  }

  key(index: number): string | null {
    return [...this.store.keys()][index] ?? null
  }

  removeItem(key: string): void {
    this.store.delete(key)
  }

  setItem(key: string, value: string): void {
    this.store.set(key, String(value))
  }
}

Object.defineProperty(globalThis, 'localStorage', {
  value: new MemoryStorage(),
  writable: true,
  configurable: true,
})
