<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import FindingToolbar from './FindingToolbar.vue'
import FindingItem from './FindingItem.vue'

const store = useScanStore()
const { t } = useI18n()

const severityOrder: Record<string, number> = { critical: 0, warning: 1, info: 2 }
</script>

<template>
  <div class="card">
    <h2>
      {{ t('report.findings') }}
      <span class="dim">
        {{ store.filteredFindings.length }} {{ t('report.findingCount') }}<span v-if="store.isCancelled">{{ t('report.partial') }}</span>
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
      {{ t('report.noFinding') }} <b>✓</b>
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