<script setup lang="ts">
import { ElMessage } from 'element-plus'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'

const store = useScanStore()
const { t } = useI18n()

async function doExport(format: string) {
  const result = await store.exportReport(format)
  if (result) {
    ElMessage.success(`${t('report.exported')} ${result.split(/[\\/]/).pop()}`)
  }
}
</script>

<template>
  <div class="export-row">
    <span class="dim">{{ t('report.export') }}</span>
    <el-button size="small" @click="doExport('html')">⬇ HTML</el-button>
    <el-button size="small" @click="doExport('md')">⬇ Markdown</el-button>
    <el-button size="small" @click="doExport('json')">⬇ JSON</el-button>
  </div>
</template>

<style scoped>
.export-row {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  align-items: center;
}
</style>
