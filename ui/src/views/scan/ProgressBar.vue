<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()

const percent = computed<number | null>(() => store.progress?.percent ?? null)

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
