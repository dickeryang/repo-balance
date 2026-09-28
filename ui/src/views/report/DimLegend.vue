<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()

function barColor(score: number | null): string {
  if (score === null) return 'var(--border)'
  if (score >= 80) return 'var(--ok)'
  if (score >= 60) return 'var(--warning)'
  return 'var(--critical)'
}
</script>

<template>
  <div class="dim-legend">
    <div
      v-for="point in store.report?.radarPoints ?? []"
      :key="point.key"
      class="dim-row"
      :class="{ active: store.activeFilter === point.key }"
      @click="store.setFilter(point.key)"
    >
      <span>{{ point.name }}</span>
      <div class="bar">
        <i :style="{ width: (point.score ?? 0) + '%', background: barColor(point.score) }"></i>
      </div>
      <span class="sc">{{ point.score ?? 'N/A' }}</span>
      <span class="fc">{{ point.findingCount }} {{ t('report.dimCount') }}</span>
    </div>
  </div>
</template>

<style scoped>
.dim-legend {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
}
.dim-row {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 9px 14px;
  font-size: 13px;
  cursor: pointer;
  transition: border-color 0.15s;
}
.dim-row:hover,
.dim-row.active {
  border-color: var(--accent);
}
.bar {
  flex: 1;
  height: 6px;
  border-radius: 999px;
  background: var(--border);
  overflow: hidden;
}
.bar i {
  display: block;
  height: 100%;
  border-radius: 999px;
}
.sc {
  width: 34px;
  text-align: right;
  font-weight: 700;
}
.fc {
  font-size: 11px;
  color: var(--text-dim);
  width: 52px;
  text-align: right;
}
</style>