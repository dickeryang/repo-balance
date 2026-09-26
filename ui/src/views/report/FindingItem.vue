<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
import type { Finding } from '../../types/models'

const props = defineProps<{ finding: Finding }>()
const store = useScanStore()

const sevText: Record<string, string> = { critical: '严重', warning: '警告', info: '提示' }
const catText: Record<string, string> = { structure: '结构', history: '历史', branches: '分支', deps: '依赖', security: '安全' }

function toggle() {
  store.toggleFinding(props.finding.id)
}

function isOpen(): boolean {
  return store.expandedFindings.has(props.finding.id)
}
</script>

<template>
  <div class="finding" :class="{ open: isOpen() }">
    <div class="finding-head" @click="toggle">
      <div class="sev-dot" :class="`sev-${finding.severity}`"></div>
      <span class="sev-tag" :class="`tag-${finding.severity}`">{{ sevText[finding.severity] }}</span>
      <span class="cat-tag">{{ catText[finding.category] }}</span>
      <span class="finding-title">{{ finding.title }}</span>
      <span class="chevron">{{ isOpen() ? '▲' : '▼' }}</span>
    </div>
    <div v-if="isOpen()" class="finding-body">
      <div class="detail-grid">
        <div class="detail-box">
          <div class="lbl">
            <span>证据</span>
            <span class="mask-note">🔒 掩码展示 · 导出同掩码</span>
          </div>
          <div class="evidence">{{ finding.evidence }}</div>
        </div>
        <div class="detail-box">
          <div class="lbl"><span>修复建议</span></div>
          <div class="suggestion">{{ finding.suggestion }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.finding {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
}
.finding-head {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 13px 16px;
  cursor: pointer;
  user-select: none;
}
.finding-head:hover {
  background: var(--bg-hover);
}
.sev-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}
.finding-title {
  font-weight: 600;
  font-size: 13px;
  flex: 1;
}
.chevron {
  color: var(--text-dim);
  font-size: 11px;
}
.finding-body {
  padding: 4px 16px 16px;
  border-top: 1px solid var(--border);
  animation: fadein 0.2s ease;
}
.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-top: 12px;
}
.detail-box {
  background: var(--bg-card);
  border-radius: 8px;
  padding: 12px 14px;
}
.lbl {
  font-size: 11px;
  color: var(--text-dim);
  margin-bottom: 6px;
  display: flex;
  justify-content: space-between;
}
.mask-note {
  color: var(--info);
  font-size: 10px;
}
.evidence {
  font-family: "SF Mono", Menlo, monospace;
  font-size: 12px;
  color: #ffd9dd;
  word-break: break-all;
  line-height: 1.7;
}
.suggestion {
  font-size: 13px;
  line-height: 1.7;
  border-left: 3px solid var(--accent);
  background: var(--accent-soft);
  padding: 10px 14px;
  border-radius: 0 8px 8px 0;
}
</style>