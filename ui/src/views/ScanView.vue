<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import ProgressBar from './scan/ProgressBar.vue'
import CheckerFeed from './scan/CheckerFeed.vue'

// 事件监听由 store 在 startScan 时统一注册，此处只做渲染。
const store = useScanStore()
</script>

<template>
  <div class="card">
    <div class="progress-head">
      <h2>正在体检 <span class="dim" style="font-weight:400;">{{ store.selectedPath?.split(/[\\/]/).pop() }}</span></h2>
      <div class="head-actions">
        <button class="btn ghost" @click="store.switchRepo()">← 返回重新选择</button>
        <button class="btn danger" :disabled="store.isCancelled" @click="store.cancelScan()">
          {{ store.isCancelled ? '取消请求已发送…' : '✕ 取消扫描' }}
        </button>
      </div>
    </div>
    <ProgressBar />
    <CheckerFeed />
  </div>
</template>

<style scoped>
.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.head-actions {
  display: flex;
  gap: 10px;
}
</style>