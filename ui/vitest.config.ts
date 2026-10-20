import { defineConfig } from 'vitest/config'

// 前端单元测试配置：当前只覆盖 store 的纯逻辑（配置 / 历史 / 趋势 / 批量调度），
// 不涉及 DOM 渲染，故用 node 环境 + 最小 localStorage 实现（见 src/test/setup.ts），
// 不引入 jsdom，保持依赖与启动开销最小。
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
    setupFiles: ['src/test/setup.ts'],
  },
})
