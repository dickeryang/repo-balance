<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import DropZone from './connect/DropZone.vue'
import RepoInfoCard from './connect/RepoInfoCard.vue'
import ConfigPanel from './connect/ConfigPanel.vue'

const store = useScanStore()
const { t } = useI18n()
</script>

<template>
  <DropZone v-if="!store.selectedPath" />
  <template v-else>
    <div v-if="store.repoError" class="error-card card">
      <div class="error-msg">⚠ {{ store.repoError }}</div>
      <button class="btn ghost" style="margin-top:14px;" @click="store.selectDirectory()">{{ t('connect.reselDir') }}</button>
    </div>
    <template v-else-if="store.repoInfo">
      <RepoInfoCard />
      <ConfigPanel />
    </template>
  </template>
</template>

<style scoped>
.error-card {
  text-align: center;
  padding: 40px 24px;
}
.error-msg {
  color: var(--critical);
  font-size: 15px;
  font-weight: 600;
}
</style>