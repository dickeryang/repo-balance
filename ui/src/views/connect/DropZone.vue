<script setup lang="ts">
import { ref } from 'vue'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()
const remoteUrl = ref('')

/** 提交远程 URL：非空时直接进入远程体检流程（浅克隆→扫描→自动清理）。 */
function submitRemote() {
  const url = remoteUrl.value.trim()
  if (url) store.startRemoteScan(url)
}
</script>

<template>
  <div class="dropzone" @click="store.selectDirectory()">
    <div class="icon">📂</div>
    <div class="title">{{ t('connect.title') }}</div>
    <p>{{ t('connect.desc') }}</p>
    <button class="btn" style="margin-top:20px;" @click.stop="store.selectDirectory()">{{ t('connect.selectDir') }}</button>
    <p class="dim" style="margin-top:14px;">{{ t('connect.demoHint') }} <code>{{ t('connect.demoName') }}</code>{{ t('connect.demoSuffix') }}</p>
    <div class="remote-row" @click.stop>
      <input
        v-model="remoteUrl"
        class="remote-input"
        type="text"
        :placeholder="t('connect.remotePlaceholder')"
        @keydown.enter="submitRemote"
      />
      <button class="btn ghost" :disabled="!remoteUrl.trim()" @click="submitRemote">{{ t('connect.remoteScan') }}</button>
    </div>
    <p class="dim">{{ t('connect.remoteHint') }}</p>
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
.remote-row {
  display: flex;
  gap: 8px;
  margin-top: 22px;
  justify-content: center;
}
.remote-input {
  width: 380px;
  max-width: 70%;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-card);
  color: var(--text);
  font-size: 13px;
}
.remote-input:focus {
  outline: none;
  border-color: var(--accent);
}
</style>