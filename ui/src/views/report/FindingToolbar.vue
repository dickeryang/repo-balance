<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
const store = useScanStore()

const severityFilters = [
  { key: 'all', label: '全部' },
  { key: 'critical', label: '严重' },
  { key: 'warning', label: '警告' },
  { key: 'info', label: '提示' },
]

const categoryFilters = [
  { key: 'structure', label: '结构' },
  { key: 'history', label: '历史' },
  { key: 'branches', label: '分支' },
  { key: 'deps', label: '依赖' },
  { key: 'security', label: '安全' },
]

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
      v-for="f in severityFilters"
      :key="f.key"
      class="filter-chip"
      :class="{ active: store.activeFilter === f.key }"
      @click="store.setFilter(f.key)"
    >
      {{ f.label }} {{ countByFilter(f.key) }}
    </button>
    <span style="margin: 0 4px; color: var(--border);">|</span>
    <button
      v-for="f in categoryFilters"
      :key="f.key"
      class="filter-chip"
      :class="{ active: store.activeFilter === f.key }"
      @click="store.setFilter(f.key)"
    >
      {{ f.label }} {{ countByFilter(f.key) }}
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