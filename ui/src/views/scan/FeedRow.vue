<script setup lang="ts">
import type { CheckerFeedRow } from '../../types/models'
defineProps<{ row: CheckerFeedRow }>()
</script>

<template>
  <div class="feed-row">
    <div class="dot" :class="row.status"></div>
    <span>{{ row.checkerId }}</span>
    <span class="cnt" v-if="row.status === 'done'">完成 · finding <b v-if="row.findingCount > 0">{{ row.findingCount }}</b><span v-else>0</span></span>
    <span class="cnt" v-else-if="row.status === 'running'">运行中…</span>
    <span class="cnt" v-else>待运行</span>
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