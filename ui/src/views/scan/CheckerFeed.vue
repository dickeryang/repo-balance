<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import FeedRow from './FeedRow.vue'

const store = useScanStore()
const { t } = useI18n()
</script>

<template>
  <div class="checker-feed">
    <FeedRow v-for="row in store.checkerFeed" :key="row.checkerId" :row="row" />
    <div v-if="store.checkerFeed.length === 0" class="dim" style="padding:12px;">
      {{ t('scan.checkerRegistered') }}
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