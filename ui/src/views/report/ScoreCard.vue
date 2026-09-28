<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
const store = useScanStore()
const { t } = useI18n()

function scoreClass(score: number): string {
  if (score >= 80) return 'good'
  if (score >= 60) return 'mid'
  return 'bad'
}

function gradeText(score: number): string {
  if (score >= 80) return t('report.health')
  if (score >= 60) return t('report.needsImprove')
  return t('report.needsAttention')
}

function gradeStyle(score: number): string {
  if (score >= 80) return 'background: rgba(79,240,122,0.12); color: var(--ok);'
  if (score >= 60) return 'background: rgba(255,180,77,0.12); color: var(--warning);'
  return 'background: rgba(255,92,108,0.12); color: var(--critical);'
}

function formatTime(ts: number): string {
  const d = new Date(ts * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}
</script>

<template>
  <div class="card score-card">
    <div class="dim">{{ t('report.summary') }}</div>
    <div class="score-big" :class="scoreClass(store.totalScore)">{{ store.totalScore }}</div>
    <div class="score-label">{{ t('report.scoreLabel') }}</div>
    <div class="score-grade" :style="gradeStyle(store.totalScore)">{{ gradeText(store.totalScore) }}</div>
    <div class="dim" style="margin-top:14px;" v-if="store.report">
      {{ store.report.repoPath.split(/[\\/]/).pop() }} · {{ formatTime(store.report.startedAt) }} · {{ (store.report.durationMs / 1000).toFixed(1) }}s
      <span v-if="store.isCancelled" style="color:var(--warning);">{{ t('report.cancelled') }}</span>
    </div>
  </div>
</template>

<style scoped>
.score-card {
  width: 240px;
  flex-shrink: 0;
  text-align: center;
}
.score-big {
  font-size: 64px;
  font-weight: 800;
  line-height: 1.1;
}
.score-big.good { color: var(--ok); }
.score-big.mid { color: var(--warning); }
.score-big.bad { color: var(--critical); }
.score-label {
  color: var(--text-dim);
  font-size: 12px;
  margin-top: 4px;
}
.score-grade {
  display: inline-block;
  margin-top: 12px;
  padding: 4px 14px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}
</style>