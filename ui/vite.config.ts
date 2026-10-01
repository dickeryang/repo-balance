import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        // 第三方依赖拆分为独立 chunk，业务代码更新时 vendor 包可长期缓存
        manualChunks(id: string) {
          if (!id.includes('node_modules')) return
          if (id.includes('element-plus') || id.includes('@element-plus')) return 'vendor-element-plus'
          if (id.includes('@vue') || id.includes('/vue/') || id.includes('pinia')) return 'vendor-vue'
          return 'vendor-other'
        },
      },
    },
  },
})