<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()

const severityKeys = ['all', 'critical', 'warning', 'info']
const categoryKeys = ['structure', 'history', 'branches', 'deps', 'security']

function countByFilter(key: string): number {
  if (!store.report) return 0
  if (key === 'all') return store.report.findings.length
  if (['critical', 'warning', 'info'].includes(key)) {
    return store.report.findings.filter((f) => f.severity === key).length
  }
  return store.report.findings.filter((f) => f.category === key).length
}
</script>

<template>
  <div class="finding-toolbar">
    <button
      v-for="key in severityKeys"
      :key="key"
      class="filter-chip"
      :class="{ active: store.activeFilter === key }"
      @click="store.setFilter(key)"
    >
      {{ t(`filter.${key}`) }} {{ countByFilter(key) }}
    </button>
    <span style="margin: 0 4px; color: var(--border);">|</span>
    <button
      v-for="key in categoryKeys"
      :key="key"
      class="filter-chip"
      :class="{ active: store.activeFilter === key }"
      @click="store.setFilter(key)"
    >
      {{ t(`filter.${key}`) }} {{ countByFilter(key) }}
    </button>
  </div>
</template>

<style scoped>
.finding-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
  flex-wrap: wrap;
}
.filter-chip {
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-dim);
  border-radius: 999px;
  padding: 5px 14px;
  font-size: 12px;
}
.filter-chip.active {
  background: var(--accent-soft);
  color: var(--accent);
  border-color: var(--accent);
}
.filter-chip:hover {
  border-color: var(--accent);
}
</style>