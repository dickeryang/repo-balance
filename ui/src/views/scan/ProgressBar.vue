<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()

// 引擎切片按检查器计划执行：big-files 的「提交历史」与「历史文件内容」是两段
// 独立采集，百分比会在 90% 处回落到 10% 再涨回。UI 只向用户承诺「整体推进」，
// 单调取最大（阶段文案仍如实显示当前阶段，可能重复出现「历史」）。
const percent = ref<number | null>(null)
watch(
  () => store.progress,
  (p) => {
    if (p === null) {
      percent.value = null
      return
    }
    if (p.percent === null) return
    percent.value = Math.max(percent.value ?? 0, p.percent)
  },
  { immediate: true },
)

const fillStyle = computed(() =>
  percent.value === null ? undefined : { width: `${percent.value}%` },
)

const stageText = computed(() => {
  const p = store.progress
  if (!p) return t('scan.preparing')
  const range = p.total > 0 ? `（${p.done}/${p.total}）` : ''
  return `${t('scan.stage')}: ${t(`scan.stage.${p.stage}`)}${range}`
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
