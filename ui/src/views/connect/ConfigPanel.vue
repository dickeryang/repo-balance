<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../../stores/scan'

const store = useScanStore()

const thresholdMiB = computed<number>({
  get: () => Math.round((store.scanConfig.bigFileThreshold / 1_048_576) * 10) / 10,
  set: (v) =>
    store.setScanConfig({
      ...store.scanConfig,
      bigFileThreshold: Math.max(0, Math.round(v * 1_048_576)),
    }),
})

const bigFilesEnabled = computed<boolean>({
  get: () => store.scanConfig.enabledCheckers.includes('big-files'),
  set: (on) => {
    const set = new Set(store.scanConfig.enabledCheckers)
    if (on) set.add('big-files')
    else set.delete('big-files')
    store.setScanConfig({ ...store.scanConfig, enabledCheckers: [...set] })
  },
})
</script>

<template>
  <div class="config-panel card">
    <h3>检查器配置</h3>
    <div class="config-row">
      <label class="toggle">
        <input type="checkbox" v-model="bigFilesEnabled" />
        <span>大文件检查器 <code>big-files</code></span>
      </label>
    </div>
    <div class="config-row">
      <label for="threshold">大文件阈值</label>
      <input id="threshold" type="number" min="0" step="0.1" v-model.number="thresholdMiB" />
      <span class="dim">MiB</span>
    </div>
    <p class="dim hint">配置保存在本地（localStorage），下次启动自动恢复</p>
  </div>
</template>

<style scoped>
.config-panel {
  margin-top: 16px;
}
.config-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}
.toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: 13px;
}
.toggle input {
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}
.config-row label {
  font-size: 13px;
}
.config-row input[type='number'] {
  width: 100px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 10px;
  color: var(--text);
  font-size: 13px;
  font-family: "SF Mono", Menlo, monospace;
}
.config-row input[type='number']:focus {
  outline: none;
  border-color: var(--accent);
}
code {
  background: var(--bg-card);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: "SF Mono", Menlo, monospace;
  font-size: 12px;
}
.hint {
  margin-top: 14px;
  font-size: 11px;
}
</style>
