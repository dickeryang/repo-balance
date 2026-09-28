<script setup lang="ts">
import { ref } from 'vue'
import { useScanStore } from '../../stores/scan'

const store = useScanStore()
const toastMsg = ref('')

async function doExport(format: string) {
  const result = await store.exportReport(format)
  if (result) {
    toastMsg.value = `已导出 ${result.split(/[\\/]/).pop()}`
    setTimeout(() => { toastMsg.value = '' }, 3000)
  }
}
</script>

<template>
  <div class="export-row">
    <span class="dim">导出报告（evidence 保持掩码）</span>
    <button class="btn ghost" @click="doExport('html')">⬇ HTML</button>
    <button class="btn ghost" @click="doExport('md')">⬇ Markdown</button>
    <button class="btn ghost" @click="doExport('json')">⬇ JSON</button>
  </div>
  <div v-if="toastMsg" class="toast">{{ toastMsg }}</div>
</template>

<style scoped>
.export-row {
  display: flex;
  gap: 12px;
  margin-top: 20px;
  justify-content: flex-end;
  align-items: center;
}
.toast {
  position: fixed;
  bottom: 30px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--bg-hover);
  border: 1px solid var(--accent);
  border-radius: 8px;
  padding: 10px 20px;
  font-size: 13px;
  z-index: 99;
}
</style>