<script setup lang="ts">
import { computed } from 'vue'
import type { CheckerFeedRow } from '../../types/models'
import { useI18n } from '../../i18n'
const props = defineProps<{ row: CheckerFeedRow }>()
const { t } = useI18n()

/** 检查器本地化名称；无翻译键时回落原始 id。 */
const label = computed(() => {
  const key = `config.checker.${props.row.checkerId}`
  const value = t(key)
  return value === key ? props.row.checkerId : value
})
</script>

<template>
  <div class="feed-row">
    <div class="dot" :class="row.status"></div>
    <span>{{ label }} <code class="row-id">{{ row.checkerId }}</code></span>
    <span class="cnt" v-if="row.status === 'done'">{{ t('scan.feedDone') }} <b v-if="row.findingCount > 0">{{ row.findingCount }}</b><span v-else>0</span></span>
    <span class="cnt" v-else-if="row.status === 'running'">{{ t('scan.feedRunning') }}</span>
    <span class="cnt" v-else>{{ t('scan.feedPending') }}</span>
  </div>
</template>

<style scoped>
.feed-row {
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 13px;
  animation: fadein 0.3s ease;
}
.row-id {
  font-family: "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: var(--text-dim);
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.dot.running {
  background: var(--accent);
  animation: pulse 1s infinite;
}
.dot.done {
  background: var(--ok);
}
.dot.pending {
  background: var(--border);
}
.cnt {
  margin-left: auto;
  color: var(--text-dim);
  font-size: 12px;
}
.cnt b {
  color: var(--warning);
}
</style>
