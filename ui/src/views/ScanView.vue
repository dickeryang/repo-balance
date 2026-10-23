<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import ProgressBar from './scan/ProgressBar.vue'
import CheckerFeed from './scan/CheckerFeed.vue'

const store = useScanStore()
const { t } = useI18n()

/** 进度页标题后缀：本地扫描显示仓库目录名，远程体检显示 URL（否则标题空白）。 */
const targetLabel = computed(
  () => store.selectedPath?.split(/[\\/]/).pop() ?? store.remoteUrl ?? '',
)
</script>

<template>
  <div class="card">
    <div class="progress-head">
      <h2>{{ t('scan.title') }} <span class="dim" style="font-weight:400;">{{ targetLabel }}</span></h2>
      <div class="head-actions">
        <el-button size="small" @click="store.switchRepo()">{{ t('scan.back') }}</el-button>
        <el-button size="small" type="danger" plain :disabled="store.isCancelled" @click="store.cancelScan()">
          {{ store.isCancelled ? t('scan.cancelling') : t('scan.cancel') }}
        </el-button>
      </div>
    </div>
    <ProgressBar />
    <CheckerFeed />
  </div>
</template>

<style scoped>
.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.head-actions {
  display: flex;
  gap: 10px;
}
</style>