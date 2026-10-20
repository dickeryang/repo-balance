<script setup lang="ts">
import { computed } from 'vue'
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import ProgressBar from './scan/ProgressBar.vue'

const store = useScanStore()
const { t } = useI18n()

/** 已结束（完成 / 跳过 / 失败）的仓库数，用于整批进度。 */
const settled = computed(
  () =>
    store.batchItems.filter((i) => ['done', 'skipped', 'failed'].includes(i.status)).length,
)

/** 整批进度百分比（按「已结束仓库数 / 队列总数」计，不做时间外推）。 */
const percent = computed(() =>
  store.batchItems.length === 0
    ? 0
    : Math.round((settled.value / store.batchItems.length) * 100),
)

/** 正在扫描的仓库名（整批串行，最多一个）。 */
const currentName = computed(
  () => store.batchItems.find((i) => i.status === 'running')?.name ?? null,
)

/** 已完成且有评分的仓库，按总分从高到低（对比看板主体）。 */
const ranked = computed(() =>
  store.batchItems
    .filter((i) => i.status === 'done' && i.total !== null)
    .sort((a, b) => (b.total ?? 0) - (a.total ?? 0)),
)

/** 评分色阶与报告页 ScoreCard 保持同一口径（≥80 健康 / ≥60 待改进 / 其余需关注）。 */
function scoreClass(score: number): string {
  if (score >= 80) return 'good'
  if (score >= 60) return 'mid'
  return 'bad'
}

function statusLabel(status: string): string {
  return t(`batch.status.${status}`)
}

/** 时长展示：<1s 用毫秒，<60s 用秒，否则「分+秒」。 */
function fmtDuration(ms: number): string {
  if (ms <= 0) return '—'
  if (ms < 1000) return `${ms}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = Math.floor(seconds / 60)
  return `${minutes}m${Math.round(seconds - minutes * 60)}s`
}
</script>

<template>
  <div v-if="store.batchItems.length === 0" class="card empty">
    <h2>{{ t('batch.title') }}</h2>
    <p class="dim">{{ t('batch.empty') }}</p>
    <el-button size="small" style="margin-top: 14px" @click="store.setCurrentView(1)">
      {{ t('batch.back') }}
    </el-button>
  </div>

  <template v-else>
    <div class="card">
      <div class="head">
        <h2>
          {{ t('batch.title') }}
          <span class="dim count">
            {{
              t('batch.progress')
                .replace('{done}', String(settled))
                .replace('{total}', String(store.batchItems.length))
            }}
          </span>
        </h2>
        <div class="actions">
          <el-button
            v-if="store.batchRunning"
            size="small"
            type="danger"
            plain
            @click="store.cancelBatch()"
          >
            {{ t('batch.cancel') }}
          </el-button>
          <template v-else>
            <el-button size="small" @click="store.clearBatch()">{{ t('batch.clear') }}</el-button>
            <el-button size="small" @click="store.switchRepo()">{{ t('batch.back') }}</el-button>
          </template>
        </div>
      </div>
      <p class="dim hint">{{ t('batch.subtitle') }}</p>

      <div class="batch-track">
        <div class="batch-fill" :style="{ width: `${percent}%` }"></div>
      </div>
      <div class="batch-meta">
        <span class="dim">
          {{
            currentName
              ? t('batch.current').replace('{name}', currentName)
              : t('batch.finished')
          }}
        </span>
        <span class="pct">{{ percent }}%</span>
      </div>

      <!-- 当前仓库的细粒度进度（复用单仓进度条与阶段文案）。 -->
      <ProgressBar v-if="store.batchRunning" />
    </div>

    <div class="card">
      <h2>{{ t('batch.summary') }}</h2>
      <div class="stats">
        <div class="stat">
          <span class="k">{{ t('batch.avgScore') }}</span>
          <span
            class="v"
            :class="store.batchSummary.avgScore !== null ? scoreClass(store.batchSummary.avgScore) : ''"
          >
            {{ store.batchSummary.avgScore ?? 'N/A' }}
          </span>
        </div>
        <div class="stat">
          <span class="k">{{ t('batch.repoCount') }}</span>
          <span class="v">
            {{ store.batchSummary.done }} / {{ store.batchSummary.total }}
          </span>
        </div>
        <div class="stat">
          <span class="k">{{ t('batch.findingsTotal') }}</span>
          <span class="v">{{ store.batchSummary.findingCount }}</span>
        </div>
        <div class="stat">
          <span class="k">{{ t('batch.criticalTotal') }}</span>
          <span class="v bad">{{ store.batchSummary.criticalCount }}</span>
        </div>
        <div class="stat">
          <span class="k">{{ t('batch.failedTotal') }}</span>
          <span class="v" :class="{ bad: store.batchSummary.failed > 0 }">
            {{ store.batchSummary.failed }}
          </span>
        </div>
        <div class="stat">
          <span class="k">{{ t('batch.elapsed') }}</span>
          <span class="v">{{ fmtDuration(store.batchSummary.durationMs) }}</span>
        </div>
      </div>

      <div v-if="ranked.length > 0" class="ranking">
        <div class="ranking-head dim">{{ t('batch.ranking') }}</div>
        <div v-for="item in ranked" :key="item.path" class="rank-row">
          <span class="rank-name" :title="item.path">{{ item.name }}</span>
          <div class="rank-bar">
            <div
              class="rank-fill"
              :class="scoreClass(item.total ?? 0)"
              :style="{ width: `${item.total ?? 0}%` }"
            ></div>
          </div>
          <span class="rank-score" :class="scoreClass(item.total ?? 0)">
            {{ item.total ?? 'N/A' }}
          </span>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>{{ t('batch.queue') }}</h2>
      <table class="queue">
        <thead>
          <tr>
            <th>{{ t('batch.name') }}</th>
            <th>{{ t('batch.status') }}</th>
            <th class="num">{{ t('batch.score') }}</th>
            <th class="num">{{ t('batch.findings') }}</th>
            <th class="num">{{ t('batch.critical') }}</th>
            <th class="num">{{ t('batch.duration') }}</th>
            <th class="num"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(item, index) in store.batchItems" :key="item.path">
            <td>
              <div class="repo-name">{{ item.name }}</div>
              <div class="path dim" :title="item.path">{{ item.path }}</div>
              <div v-if="item.error" class="err">
                {{ t('batch.failedReason') }}: {{ item.error }}
              </div>
            </td>
            <td>
              <span class="badge" :class="item.status">{{ statusLabel(item.status) }}</span>
            </td>
            <td class="num" :class="item.total !== null ? scoreClass(item.total) : ''">
              {{ item.total ?? '—' }}
            </td>
            <td class="num">{{ item.status === 'done' ? item.findingCount : '—' }}</td>
            <td class="num">{{ item.status === 'done' ? item.criticalCount : '—' }}</td>
            <td class="num">{{ fmtDuration(item.durationMs) }}</td>
            <td class="num">
              <el-button v-if="item.report" size="small" link @click="store.openBatchReport(index)">
                {{ t('batch.view') }}
              </el-button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </template>
</template>

<style scoped>
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.head .count {
  font-size: 12px;
  font-weight: 400;
}
.actions {
  display: flex;
  gap: 8px;
}
.hint {
  font-size: 11px;
  margin-top: 6px;
}
.batch-track {
  height: 8px;
  background: var(--bg-card);
  border-radius: 999px;
  overflow: hidden;
  margin-top: 16px;
}
.batch-fill {
  height: 100%;
  background: linear-gradient(90deg, #4f8cff, #7a5cff);
  border-radius: 999px;
  transition: width 0.4s ease;
}
.batch-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
  font-size: 12px;
}
.pct {
  font-weight: 700;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: 12px;
  margin-top: 14px;
}
.stat {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.stat .k {
  font-size: 11px;
  color: var(--text-dim);
}
.stat .v {
  font-size: 22px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.v.good,
.rank-score.good,
.num.good {
  color: var(--ok);
}
.v.mid,
.rank-score.mid,
.num.mid {
  color: var(--warning);
}
.v.bad,
.rank-score.bad,
.num.bad {
  color: var(--critical);
}
.ranking {
  margin-top: 18px;
}
.ranking-head {
  font-size: 11px;
  margin-bottom: 8px;
}
.rank-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 0;
}
.rank-name {
  width: 180px;
  flex-shrink: 0;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rank-bar {
  flex: 1;
  height: 8px;
  background: var(--bg-card);
  border-radius: 999px;
  overflow: hidden;
}
.rank-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.4s ease;
}
.rank-fill.good {
  background: var(--ok);
}
.rank-fill.mid {
  background: var(--warning);
}
.rank-fill.bad {
  background: var(--critical);
}
.rank-score {
  width: 40px;
  text-align: right;
  font-size: 12px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.queue {
  width: 100%;
  border-collapse: collapse;
  margin-top: 10px;
  font-size: 12px;
}
.queue th {
  text-align: left;
  font-weight: 600;
  color: var(--text-dim);
  font-size: 11px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
}
.queue td {
  padding: 10px;
  border-bottom: 1px solid var(--border);
  vertical-align: top;
}
.queue tr:last-child td {
  border-bottom: none;
}
.queue .num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.repo-name {
  font-weight: 600;
}
.path {
  font-size: 11px;
  word-break: break-all;
}
.err {
  font-size: 11px;
  color: var(--critical);
  margin-top: 4px;
}
.badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  border: 1px solid var(--border);
  color: var(--text-dim);
  white-space: nowrap;
}
.badge.running {
  color: var(--accent);
  border-color: var(--accent);
}
.badge.done {
  color: var(--ok);
  border-color: rgba(79, 240, 122, 0.4);
}
.badge.failed {
  color: var(--critical);
  border-color: rgba(255, 92, 108, 0.4);
}
.badge.skipped {
  color: var(--warning);
  border-color: rgba(255, 180, 77, 0.4);
}
.empty p {
  margin-top: 10px;
  font-size: 13px;
}
</style>
