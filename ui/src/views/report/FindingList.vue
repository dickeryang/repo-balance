<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import FindingToolbar from './FindingToolbar.vue'
import FindingItem from './FindingItem.vue'

const store = useScanStore()

const severityOrder: Record<string, number> = { critical: 0, warning: 1, info: 2 }
</script>

<template>
  <div class="card">
    <h2>
      发现的问题
      <span class="dim">
        {{ store.filteredFindings.length }} 条<span v-if="store.isCancelled">（部分结果）</span>
      </span>
    </h2>
    <FindingToolbar />
    <div class="finding-list" v-if="store.filteredFindings.length > 0">
      <FindingItem
        v-for="f in [...store.filteredFindings].sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity])"
        :key="f.id"
        :finding="f"
      />
    </div>
    <div v-else class="healthy-dim">
      该筛选条件下无发现 · 维度健康 <b>✓</b>
    </div>
  </div>
</template>

<style scoped>
.finding-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.healthy-dim {
  text-align: center;
  padding: 8px;
  color: var(--text-dim);
  font-size: 12px;
}
.healthy-dim b {
  color: var(--ok);
}
</style>