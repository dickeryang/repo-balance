<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import { ALL_CHECKER_IDS } from '../../types/models'

const store = useScanStore()
const { t } = useI18n()

/** 检查器 id → i18n 标签键（与后端 ALL_CHECKER_IDS 顺序一致）。 */
const CHECKER_LABEL_KEYS: Record<string, string> = {
  'big-files': 'config.checker.bigFiles',
  secrets: 'config.checker.secrets',
  'zombie-branches': 'config.checker.zombieBranches',
  'commit-habits': 'config.checker.commitHabits',
  'deps-health': 'config.checker.depsHealth',
}

const thresholdMiB = computed<number>({
  get: () => Math.round((store.scanConfig.bigFileThreshold / 1_048_576) * 10) / 10,
  set: (v) =>
    store.setScanConfig({
      ...store.scanConfig,
      bigFileThreshold: Math.max(0, Math.round(v * 1_048_576)),
    }),
})

const zombieDays = computed<number>({
  get: () => store.scanConfig.zombieBranchDays,
  set: (v) =>
    store.setScanConfig({
      ...store.scanConfig,
      zombieBranchDays: Math.max(1, Math.round(v) || 1),
    }),
})

function checkerEnabled(id: string): boolean {
  // 空列表 = 全部启用：任一开关缺失时按启用展示，保持 UI 与后端口径一致。
  const list = store.scanConfig.enabledCheckers
  return list.length === 0 || list.includes(id)
}

function setCheckerEnabled(id: string, on: boolean) {
  const current = store.scanConfig.enabledCheckers
  const base = current.length > 0 ? [...current] : [...ALL_CHECKER_IDS]
  const next = on ? [...new Set([...base, id])] : base.filter((c) => c !== id)
  store.setScanConfig({ ...store.scanConfig, enabledCheckers: next })
}
</script>

<template>
  <div class="config-panel card">
    <h3>{{ t('config.title') }}</h3>
    <div v-for="id in ALL_CHECKER_IDS" :key="id" class="config-row">
      <label class="toggle">
        <input type="checkbox" :checked="checkerEnabled(id)" @change="setCheckerEnabled(id, ($event.target as HTMLInputElement).checked)" />
        <span>{{ t(CHECKER_LABEL_KEYS[id]) }} <code>{{ id }}</code></span>
      </label>
    </div>
    <div class="config-row">
      <label for="threshold">{{ t('config.threshold') }}</label>
      <input id="threshold" type="number" min="0" step="0.1" v-model.number="thresholdMiB" />
      <span class="dim">MiB</span>
    </div>
    <div class="config-row">
      <label for="zombie-days">{{ t('config.zombieDays') }}</label>
      <input id="zombie-days" type="number" min="1" step="1" v-model.number="zombieDays" />
      <span class="dim">{{ t('config.zombieDaysUnit') }}</span>
    </div>
    <p class="dim hint">{{ t('config.hint') }}</p>
    <p class="dim hint">
      {{ t('config.ignoreHint') }} <code>.repobalance-ignore</code> {{ t('config.ignoreHint2') }}
    </p>
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
