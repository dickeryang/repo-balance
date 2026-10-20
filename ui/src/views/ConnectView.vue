<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import DropZone from './connect/DropZone.vue'
import RepoInfoCard from './connect/RepoInfoCard.vue'
import ScaleEstimateCard from './connect/ScaleEstimateCard.vue'

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
      <ScaleEstimateCard />
      <!-- 检查器开关 / 阈值 / 历史范围 / 自定义规则已拆到独立设置页（视图 6）。 -->
      <p class="dim config-hint">
        {{ t('connect.configHint') }}
        <button class="btn ghost" @click="store.setCurrentView(6)">{{ t('app.settings') }}</button>
      </p>
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
.config-hint {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 16px;
  font-size: 12px;
}
</style>