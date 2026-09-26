<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
const store = useScanStore()
</script>

<template>
  <div class="progress-track">
    <div class="progress-fill" :class="{ indeterminate: !store.progress }"></div>
  </div>
  <div class="dim" style="margin-top:8px;">
    {{ store.progress ? `阶段：${store.progress.stage}（${store.progress.done}/${store.progress.total}）` : '扫描中…' }}
  </div>
</template>

<style scoped>
.progress-track {
  height: 10px;
  background: var(--bg-card);
  border-radius: 999px;
  overflow: hidden;
  margin: 18px 0 8px;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #4f8cff, #7a5cff);
  border-radius: 999px;
  transition: width 0.3s ease;
}
.progress-fill.indeterminate {
  width: 30%;
  animation: indeterminate 1.5s infinite ease-in-out;
}
@keyframes indeterminate {
  0% { margin-left: -30%; }
  100% { margin-left: 100%; }
}
</style>