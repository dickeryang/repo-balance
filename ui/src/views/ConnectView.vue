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
  <!-- 错误卡片与接入入口并列渲染：远程体检失败后（repoError 非空）用户仍能
       直接改 URL 重试或改选本地目录，而不是只看到一张错误卡、必须先点
       「回到接入页」才能找回输入框。 -->
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
  <DropZone v-if="!store.selectedPath" />
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