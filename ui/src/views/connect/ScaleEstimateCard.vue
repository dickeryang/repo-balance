<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'

const store = useScanStore()
const { t } = useI18n()

const est = computed(() => store.scaleEstimate)

/** 当前是否启用了历史限制（用于提示「本次为受限扫描」）。 */
const isLimited = computed(() => {
  const cfg = store.scanConfig
  return (cfg.maxCommits ?? 0) > 0 || (cfg.sinceDays ?? 0) > 0
})

const hasSuggestion = computed(() => {
  const e = est.value
  return !!e && (e.suggestedMaxCommits !== null || e.suggestedSinceDays !== null)
})

const alreadyApplied = computed(() => {
  const e = est.value
  if (!e) return false
  const cfg = store.scanConfig
  return cfg.maxCommits === e.suggestedMaxCommits && cfg.sinceDays === e.suggestedSinceDays
})

function fmtBytes(bytes: number): string {
  const mib = bytes / 1_048_576
  if (mib >= 1024) return `${(mib / 1024).toFixed(1)} GiB`
  if (mib >= 1) return `${mib.toFixed(1)} MiB`
  return `${Math.max(1, Math.round(bytes / 1024))} KiB`
}

function fmtDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return s === 0 ? `${m}m` : `${m}m ${s}s`
}

function fmtObjects(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}
</script>

<template>
  <div v-if="est" class="estimate-card card" :class="`level-${est.level}`">
    <div class="est-head">
      <h3>{{ t('estimate.title') }}</h3>
      <span class="level-tag">{{ t(`estimate.level.${est.level}`) }}</span>
    </div>

    <div class="est-grid">
      <div class="est-item">
        <div class="est-label">{{ t('estimate.commits') }}</div>
        <div class="est-value">{{ est.commitCount.toLocaleString() }}</div>
      </div>
      <div class="est-item">
        <div class="est-label">{{ t('estimate.files') }}</div>
        <div class="est-value">{{ est.fileCount.toLocaleString() }}</div>
      </div>
      <div class="est-item">
        <div class="est-label">{{ t('estimate.size') }}</div>
        <div class="est-value">{{ fmtBytes(est.totalBytes) }}</div>
      </div>
      <div class="est-item">
        <div class="est-label">{{ t('estimate.estSeconds') }}</div>
        <div class="est-value">≈ {{ fmtDuration(est.estimatedSeconds) }}</div>
      </div>
    </div>

    <p class="dim est-note">
      {{ t('estimate.note') }} {{ fmtObjects(est.estimatedObjects) }} {{ t('estimate.objectsUnit') }}
    </p>

    <div v-if="hasSuggestion" class="est-suggest">
      <span class="suggest-text">
        ⚠ {{ t('estimate.suggestion') }}
        <template v-if="est.suggestedMaxCommits">
          {{ t('config.maxCommits') }} {{ est.suggestedMaxCommits.toLocaleString() }}
        </template>
        <template v-if="est.suggestedMaxCommits && est.suggestedSinceDays"> · </template>
        <template v-if="est.suggestedSinceDays">
          {{ t('config.sinceDays') }} {{ est.suggestedSinceDays }}
        </template>
      </span>
      <button v-if="!alreadyApplied" class="btn ghost small" @click="store.applySuggestedLimits()">
        {{ t('estimate.apply') }}
      </button>
      <span v-else class="dim applied">✔ {{ t('estimate.applied') }}</span>
    </div>

    <p v-else-if="isLimited" class="dim est-note">
      ⚠ {{ t('estimate.limitedNote') }}
    </p>
  </div>
</template>

<style scoped>
.estimate-card {
  margin-top: 16px;
  border-left: 3px solid var(--accent);
}
.estimate-card.level-large {
  border-left-color: var(--warning);
}
.estimate-card.level-huge {
  border-left-color: var(--critical);
}
.est-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.level-tag {
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  color: var(--text-dim);
}
.level-large .level-tag {
  color: var(--warning);
  border-color: var(--warning);
}
.level-huge .level-tag {
  color: var(--critical);
  border-color: var(--critical);
}
.est-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}
.est-item {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
}
.est-label {
  font-size: 11px;
  color: var(--text-dim);
  margin-bottom: 4px;
}
.est-value {
  font-size: 15px;
  font-weight: 700;
  font-family: "SF Mono", Menlo, monospace;
}
.est-note {
  margin-top: 12px;
  font-size: 11px;
}
.est-suggest {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(255, 176, 32, 0.1);
  border: 1px solid rgba(255, 176, 32, 0.35);
}
.suggest-text {
  flex: 1;
  font-size: 12px;
}
.btn.small {
  padding: 5px 12px;
  font-size: 12px;
  flex-shrink: 0;
}
.applied {
  font-size: 12px;
  flex-shrink: 0;
}
</style>
