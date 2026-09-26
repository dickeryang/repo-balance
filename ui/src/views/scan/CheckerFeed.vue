<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import FeedRow from './FeedRow.vue'

const store = useScanStore()
</script>

<template>
  <div class="checker-feed">
    <FeedRow v-for="row in store.checkerFeed" :key="row.checkerId" :row="row" />
    <div v-if="store.checkerFeed.length === 0" class="dim" style="padding:12px;">
      已注册检查器：big-files（stub 阶段无实时进度）
    </div>
  </div>
</template>

<style scoped>
.checker-feed {
  margin-top: 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 300px;
  overflow-y: auto;
}
</style>