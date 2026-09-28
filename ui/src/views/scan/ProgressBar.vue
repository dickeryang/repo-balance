<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../../stores/scan'
const store = useScanStore()

/**
 * 进度百分比（0-100）。
 * 后端 progress 事件统一携带 percent（阶段分段估算：初始化 5% →
 * 工作区采集 10% → 历史采集 10-90% 按提交比例推进 → 检查 90-99% → 完成 100%）。
 * 尚未收到任何进度事件时为 null，进度条进入 indeterminate 待机动画。
 */
const percent = computed<number | null>(() => store.progress?.percent ?? null)

const fillStyle = computed(() =>
  percent.value === null ? undefined : { width: `${percent.value}%` },
)

const stageText = computed(() => {
  const p = store.progress
  if (!p) return '正在准备扫描…'
  const range = p.total > 0 ? `（${p.done}/${p.total}）` : ''
  return `阶段：${p.stage}${range}`
})
</script>

<template>
  <div class="progress-track">
    <div
      class="progress-fill"
      :class="{ indeterminate: percent === null }"
      :style="fillStyle"
    ></div>
  </div>
  <div class="progress-meta">
    <span class="dim">{{ stageText }}</span>
    <span v-if="percent !== null" class="pct">{{ percent }}%</span>
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
  width: 0;
  background: linear-gradient(90deg, #4f8cff, #7a5cff);
  border-radius: 999px;
  transition: width 0.4s ease;
}
.progress-fill.indeterminate {
  width: 30%;
  animation: indeterminate 1.5s infinite ease-in-out;
}
@keyframes indeterminate {
  0% { margin-left: -30%; }
  100% { margin-left: 100%; }
}
.progress-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
}
.pct {
  font-size: 12px;
  font-weight: 700;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
</style>
