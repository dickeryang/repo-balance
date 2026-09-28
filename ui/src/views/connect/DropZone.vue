<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()
</script>

<template>
  <div class="dropzone" @click="store.selectDirectory()">
    <div class="icon">📂</div>
    <div class="title">{{ t('connect.title') }}</div>
    <p>{{ t('connect.desc') }}</p>
    <button class="btn" style="margin-top:20px;" @click.stop="store.selectDirectory()">{{ t('connect.selectDir') }}</button>
    <p class="dim" style="margin-top:14px;">{{ t('connect.demoHint') }} <code>{{ t('connect.demoName') }}</code>{{ t('connect.demoSuffix') }}</p>
  </div>
</template>

<style scoped>
.dropzone {
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  padding: 64px 24px;
  text-align: center;
  transition: all 0.2s;
  background: var(--bg-panel);
  cursor: pointer;
}
.dropzone:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.icon {
  font-size: 44px;
  margin-bottom: 14px;
}
.title {
  font-size: 16px;
  font-weight: 600;
}
.dropzone p {
  color: var(--text-dim);
  font-size: 13px;
  margin-top: 8px;
}
code {
  background: var(--bg-card);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: "SF Mono", Menlo, monospace;
}
</style>