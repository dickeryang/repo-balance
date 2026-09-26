<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useScanStore } from '../stores/scan'
import { onScanDone, onScanProgress, onScanCancelled } from '../ipc/events'
import ProgressBar from './scan/ProgressBar.vue'
import CheckerFeed from './scan/CheckerFeed.vue'

const store = useScanStore()

let unlistenDone: (() => void) | null = null
let unlistenProgress: (() => void) | null = null
let unlistenCancelled: (() => void) | null = null

onMounted(async () => {
  unlistenDone = await onScanDone((report) => {
    store.setReport(report)
    store.setCurrentView(3)
  })
  unlistenProgress = await onScanProgress((progress) => {
    store.setProgress(progress)
  })
  unlistenCancelled = await onScanCancelled((error) => {
    store.isCancelled = true
    store.repoError = error
  })
})

onUnmounted(() => {
  unlistenDone?.()
  unlistenProgress?.()
  unlistenCancelled?.()
})
</script>

<template>
  <div class="card">
    <div class="progress-head">
      <h2>正在体检 <span class="dim" style="font-weight:400;">{{ store.selectedPath?.split(/[\\/]/).pop() }}</span></h2>
      <button class="btn danger" :disabled="store.isCancelled" @click="store.cancelScan()">
        {{ store.isCancelled ? '取消请求已发送…' : '✕ 取消扫描' }}
      </button>
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
</style>