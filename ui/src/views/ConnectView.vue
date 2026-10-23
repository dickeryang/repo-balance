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
  <!-- 错误卡片必须独立于「是否已选仓库」渲染：远程体检失败时 selectedPath 为空，
       若按旧写法只在选过仓库时显示错误，用户只会看到「回到接入页」而不知失败原因。 -->
  <div v-if="store.repoError" class="error-card card">
    <div class="error-msg">⚠ {{ store.repoError }}</div>
    <div class="error-actions">
      <button v-if="store.selectedPath" class="btn ghost" @click="store.selectDirectory()">
        {{ t('connect.reselDir') }}
      </button>
      <button class="btn ghost" @click="store.resetToConnect()">
        {{ t('connect.backToConnect') }}
      </button>
    </div>
  </div>
  <DropZone v-else-if="!store.selectedPath" />
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
.error-actions {
  display: flex;
  gap: 10px;
  justify-content: center;
  margin-top: 14px;
}
.config-hint {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 16px;
  font-size: 12px;
}
</style>