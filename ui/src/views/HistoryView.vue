<script setup lang="ts">
import { ref, computed } from 'vue'
import { useScanStore } from '../stores/scan'
import type { Finding } from '../types/models'

const store = useScanStore()

const baselineId = ref<string | null>(null)
const targetId = ref<string | null>(null)

const compare = computed(() => {
  if (!baselineId.value || !targetId.value) return null
  if (baselineId.value === targetId.value) return null
  return store.compareHistory(baselineId.value, targetId.value)
})

const sevText: Record<string, string> = { critical: '严重', warning: '警告', info: '提示' }
const catText: Record<string, string> = { structure: '结构', history: '历史', branches: '分支', deps: '依赖', security: '安全' }

function fmtTime(ms: number): string {
  const d = new Date(ms)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function scoreColor(score: number): string {
  if (score >= 80) return 'var(--ok)'
  if (score >= 60) return 'var(--warning)'
  return 'var(--critical)'
}

function setBaseline(id: string) {
  baselineId.value = baselineId.value === id ? null : id
}

function setTarget(id: string) {
  targetId.value = targetId.value === id ? null : id
}

function clearSelection() {
  baselineId.value = null
  targetId.value = null
}
</script>

<template>
  <div class="history-view">
    <div class="card">
      <div class="head-row">
        <h2>扫描历史与对比</h2>
        <div class="head-actions">
          <span class="dim">共 {{ store.history.length }} 条（最多保留 20 条）</span>
          <button v-if="store.history.length > 0" class="btn danger small" @click="store.clearHistory(); clearSelection()">
            清空全部
          </button>
        </div>
      </div>

      <div v-if="store.history.length === 0" class="empty">
        <p>暂无扫描历史。</p>
        <p class="dim">完成一次扫描后，报告会自动保存到此处，可用于跨次对比。</p>
      </div>

      <div v-else class="history-list">
        <div
          v-for="entry in store.history"
          :key="entry.id"
          class="history-row"
          :class="{
            'is-baseline': baselineId === entry.id,
            'is-target': targetId === entry.id,
          }"
        >
          <div class="row-main">
            <div class="row-score" :style="{ color: scoreColor(entry.totalScore) }">
              {{ entry.totalScore }}
            </div>
            <div class="row-info">
              <div class="row-name">{{ entry.repoName }}</div>
              <div class="row-meta dim">
                {{ fmtTime(entry.savedAt) }} · {{ entry.findingCount }} 项发现 · {{ entry.repoPath }}
              </div>
            </div>
          </div>
          <div class="row-actions">
            <button
              class="btn ghost tiny"
              :class="{ selected: baselineId === entry.id }"
              @click="setBaseline(entry.id)"
            >
              基准
            </button>
            <button
              class="btn ghost tiny"
              :class="{ selected: targetId === entry.id }"
              @click="setTarget(entry.id)"
            >
              对照
            </button>
            <button class="btn danger tiny" @click="store.deleteHistory(entry.id)">删除</button>
          </div>
        </div>
      </div>
    </div>

    <div v-if="compare" class="card compare-panel">
      <div class="compare-head">
        <h2>对比结果</h2>
        <button class="btn ghost small" @click="clearSelection">清除选择</button>
      </div>

      <div class="compare-summary">
        <div class="summary-item">
          <div class="summary-label">基准</div>
          <div class="summary-value">{{ compare.baseline.repoName }}</div>
          <div class="summary-sub dim">{{ fmtTime(compare.baseline.savedAt) }} · 得分 {{ compare.baseline.totalScore }}</div>
        </div>
        <div class="summary-arrow">→</div>
        <div class="summary-item">
          <div class="summary-label">对照</div>
          <div class="summary-value">{{ compare.target.repoName }}</div>
          <div class="summary-sub dim">{{ fmtTime(compare.target.savedAt) }} · 得分 {{ compare.target.totalScore }}</div>
        </div>
      </div>

      <div class="delta-row">
        <div class="delta-box" :class="compare.scoreDelta >= 0 ? 'positive' : 'negative'">
          <span class="delta-label">总分变化</span>
          <span class="delta-value">{{ compare.scoreDelta >= 0 ? '+' : '' }}{{ compare.scoreDelta }}</span>
        </div>
        <div class="delta-box" :class="compare.findingCountDelta <= 0 ? 'positive' : 'negative'">
          <span class="delta-label">发现项变化</span>
          <span class="delta-value">{{ compare.findingCountDelta >= 0 ? '+' : '' }}{{ compare.findingCountDelta }}</span>
        </div>
        <div class="delta-box neutral">
          <span class="delta-label">新增</span>
          <span class="delta-value">{{ compare.addedFindings.length }}</span>
        </div>
        <div class="delta-box neutral">
          <span class="delta-label">消除</span>
          <span class="delta-value">{{ compare.resolvedFindings.length }}</span>
        </div>
        <div class="delta-box neutral">
          <span class="delta-label">仍存在</span>
          <span class="delta-value">{{ compare.commonFindings.length }}</span>
        </div>
      </div>

      <div class="compare-sections">
        <div v-if="compare.addedFindings.length > 0" class="compare-section">
          <h3 class="section-title added">新增发现（{{ compare.addedFindings.length }}）</h3>
          <div class="finding-list">
            <div v-for="f in compare.addedFindings" :key="f.id" class="mini-finding">
              <span class="sev-dot" :class="`sev-${f.severity}`"></span>
              <span class="sev-tag" :class="`tag-${f.severity}`">{{ sevText[f.severity] }}</span>
              <span class="cat-tag">{{ catText[f.category] }}</span>
              <span class="finding-title">{{ f.title }}</span>
            </div>
          </div>
        </div>

        <div v-if="compare.resolvedFindings.length > 0" class="compare-section">
          <h3 class="section-title resolved">已消除（{{ compare.resolvedFindings.length }}）</h3>
          <div class="finding-list">
            <div v-for="f in compare.resolvedFindings" :key="f.id" class="mini-finding resolved">
              <span class="sev-dot" :class="`sev-${f.severity}`"></span>
              <span class="sev-tag" :class="`tag-${f.severity}`">{{ sevText[f.severity] }}</span>
              <span class="cat-tag">{{ catText[f.category] }}</span>
              <span class="finding-title">{{ f.title }}</span>
            </div>
          </div>
        </div>

        <div v-if="compare.commonFindings.length > 0" class="compare-section">
          <h3 class="section-title common">仍存在（{{ compare.commonFindings.length }}）</h3>
          <div class="finding-list">
            <div v-for="f in compare.commonFindings" :key="f.id" class="mini-finding">
              <span class="sev-dot" :class="`sev-${f.severity}`"></span>
              <span class="sev-tag" :class="`tag-${f.severity}`">{{ sevText[f.severity] }}</span>
              <span class="cat-tag">{{ catText[f.category] }}</span>
              <span class="finding-title">{{ f.title }}</span>
            </div>
          </div>
        </div>

        <div
          v-if="compare.addedFindings.length === 0 && compare.resolvedFindings.length === 0 && compare.commonFindings.length === 0"
          class="no-diff dim"
        >
          两次扫描的发现项完全一致，无差异。
        </div>
      </div>
    </div>

    <div v-else-if="baselineId || targetId" class="card hint">
      <p class="dim">请再选择一条作为{{ baselineId ? '对照' : '基准' }}，即可生成对比结果。</p>
      <p v-if="baselineId && targetId && baselineId === targetId" class="dim warn">
        基准与对照不能为同一条记录。
      </p>
    </div>

    <div style="margin-top:20px; display:flex; justify-content:center; gap:12px;">
      <button class="btn ghost" @click="store.setCurrentView(store.report ? 3 : 1)">← 返回</button>
    </div>
  </div>
</template>

<style scoped>
.history-view {
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.head-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}
.head-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.btn.small {
  padding: 5px 12px;
  font-size: 12px;
}
.btn.tiny {
  padding: 4px 10px;
  font-size: 11px;
}
.btn.tiny.selected {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}
.empty {
  text-align: center;
  padding: 40px 0;
  line-height: 2;
}
.history-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.history-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 16px;
  transition: border-color 0.15s;
}
.history-row.is-baseline {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
}
.history-row.is-target {
  border-color: var(--info);
  box-shadow: 0 0 0 1px var(--info);
}
.history-row.is-baseline.is-target {
  border-color: var(--warning);
  box-shadow: 0 0 0 1px var(--warning);
}
.row-main {
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 1;
  min-width: 0;
}
.row-score {
  font-size: 22px;
  font-weight: 800;
  min-width: 36px;
  text-align: center;
}
.row-info {
  min-width: 0;
}
.row-name {
  font-weight: 600;
  font-size: 14px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row-meta {
  font-size: 12px;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.compare-panel {
  animation: fadein 0.25s ease;
}
.compare-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}
.compare-summary {
  display: flex;
  align-items: center;
  gap: 20px;
  background: var(--bg-card);
  border-radius: 10px;
  padding: 16px 20px;
  margin-bottom: 16px;
}
.summary-item {
  flex: 1;
}
.summary-label {
  font-size: 11px;
  color: var(--text-dim);
  margin-bottom: 4px;
}
.summary-value {
  font-size: 16px;
  font-weight: 700;
}
.summary-sub {
  font-size: 12px;
  margin-top: 2px;
}
.summary-arrow {
  font-size: 22px;
  color: var(--text-dim);
}
.delta-row {
  display: flex;
  gap: 12px;
  margin-bottom: 20px;
  flex-wrap: wrap;
}
.delta-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px 20px;
  min-width: 90px;
}
.delta-box.positive {
  border-color: rgba(79, 208, 122, 0.5);
}
.delta-box.negative {
  border-color: rgba(255, 92, 108, 0.5);
}
.delta-box.neutral {
  border-color: var(--border);
}
.delta-label {
  font-size: 11px;
  color: var(--text-dim);
}
.delta-value {
  font-size: 20px;
  font-weight: 800;
}
.delta-box.positive .delta-value {
  color: var(--ok);
}
.delta-box.negative .delta-value {
  color: var(--critical);
}

.compare-sections {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.section-title {
  padding: 6px 12px;
  border-radius: 6px;
  display: inline-block;
  margin-bottom: 10px;
}
.section-title.added {
  color: var(--critical);
  background: rgba(255, 92, 108, 0.1);
}
.section-title.resolved {
  color: var(--ok);
  background: rgba(79, 208, 122, 0.1);
}
.section-title.common {
  color: var(--text-dim);
  background: var(--bg-card);
}
.finding-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.mini-finding {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 12px;
}
.mini-finding.resolved {
  opacity: 0.6;
  text-decoration: line-through;
  text-decoration-color: var(--text-dim);
}
.mini-finding .sev-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.mini-finding .finding-title {
  font-size: 13px;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.no-diff {
  text-align: center;
  padding: 24px;
}
.hint {
  text-align: center;
  padding: 20px;
  line-height: 2;
}
.hint .warn {
  color: var(--warning);
}
</style>
