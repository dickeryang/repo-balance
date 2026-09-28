<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import InfoItem from './InfoItem.vue'

const store = useScanStore()
const { t } = useI18n()

function resetSelection() {
  store.selectedPath = null
  store.repoInfo = null
  store.repoError = null
}

function startScan() {
  if (store.selectedPath) {
    store.setCurrentView(2)
    store.startScan(store.selectedPath)
  }
}
</script>

<template>
  <div class="repo-info card">
    <div class="repo-path">
      <span class="ok">✔</span>
      <span>{{ store.selectedPath }}</span>
      <span class="spacer"></span>
      <span class="dim">git · {{ store.repoInfo?.branches[0] ?? 'main' }}</span>
    </div>
    <div class="info-grid">
      <InfoItem :label="t('connect.commitCount')" :value="store.repoInfo?.commitCount ?? 0" />
      <InfoItem :label="t('connect.branchCount')" :value="store.repoInfo?.branchCount ?? 0" />
      <InfoItem :label="t('connect.workdirFiles')" :value="store.repoInfo?.fileCount ?? 0" />
      <InfoItem :label="t('connect.depManifests')" :value="store.repoInfo?.depManifests.length ?? 0" />
    </div>
    <div class="cta-row">
      <button class="btn ghost" @click="resetSelection">{{ t('connect.resel') }}</button>
      <button class="btn" @click="startScan">{{ t('connect.startScan') }}</button>
    </div>
  </div>
</template>

<style scoped>
.repo-path {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px 16px;
  font-family: "SF Mono", Menlo, monospace;
  font-size: 13px;
}
.repo-path .ok {
  color: var(--ok);
}
.spacer {
  flex: 1;
}
.info-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-top: 14px;
}
.cta-row {
  display: flex;
  gap: 12px;
  margin-top: 22px;
  justify-content: flex-end;
}
</style>