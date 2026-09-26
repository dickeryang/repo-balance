<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import InfoItem from './InfoItem.vue'

const store = useScanStore()

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
      <InfoItem label="提交总数" :value="store.repoInfo?.commitCount ?? 0" />
      <InfoItem label="分支数" :value="store.repoInfo?.branchCount ?? 0" />
      <InfoItem label="工作区文件" :value="store.repoInfo?.fileCount ?? 0" />
      <InfoItem label="依赖清单" :value="store.repoInfo?.depManifests.length ?? 0" />
    </div>
    <div class="cta-row">
      <button class="btn ghost" @click="resetSelection">重新选择</button>
      <button class="btn" @click="startScan">开始体检 →</button>
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