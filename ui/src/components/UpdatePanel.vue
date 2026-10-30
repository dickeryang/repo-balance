<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from 'vue'
import { useI18n } from '../i18n'
import { formatBytes, useUpdater } from '../composables/useUpdater'

/**
 * 软件更新面板。
 *
 * 这是应用内唯一的联网入口：不自动轮询，用户点「检查更新」或显式开启
 * 「启动时自动检查」后才会访问更新端点（仅取版本元数据，不带仓库信息）。
 */
const { t } = useI18n()
const updater = useUpdater()
const { phase, currentVersion, pending, received, percent, error, autoCheck, busy } = updater

const autoCheckModel = computed({
  get: () => autoCheck.value,
  set: (value: boolean) => updater.setAutoCheck(value),
})

/** 状态条文案：所有分支都由 phase 决定，避免模板里散落条件。 */
const statusText = computed(() => {
  switch (phase.value) {
    case 'checking':
      return t('update.checking')
    case 'latest':
      return t('update.latest')
    case 'available':
      return t('update.available').replace('{v}', pending.value?.version ?? '')
    case 'downloading':
      return percent.value === null
        ? t('update.downloadingUnknown').replace('{size}', formatBytes(received.value))
        : t('update.downloading').replace('{percent}', String(percent.value))
    case 'ready':
      return t('update.ready')
    case 'error':
      return t('update.error')
    default:
      return t('update.idle')
  }
})

const statusTone = computed(() => {
  if (phase.value === 'error') return 'err'
  if (phase.value === 'available' || phase.value === 'ready') return 'ok'
  return 'muted'
})

/** RFC 3339 → 本地「年-月-日」；缺失或非法时显示占位符。 */
function fmtDate(value: string | null): string {
  if (!value) return '—'
  const d = new Date(value)
  if (Number.isNaN(d.getTime())) return '—'
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

onMounted(() => {
  void updater.loadVersion()
  if (autoCheck.value) void updater.checkNow()
})

onBeforeUnmount(updater.dispose)
</script>

<template>
  <div class="card">
    <h2>{{ t('update.title') }}</h2>
    <p class="dim desc">{{ t('update.desc') }}</p>

    <div class="status">
      <span class="badge" :class="statusTone">{{ statusText }}</span>
      <span class="dim">{{ t('update.current').replace('{v}', currentVersion || '—') }}</span>
    </div>

    <div class="actions">
      <el-button size="small" :loading="phase === 'checking'" :disabled="busy" @click="updater.checkNow">
        {{ t('update.check') }}
      </el-button>
      <el-button v-if="phase === 'available'" type="primary" size="small" @click="updater.installNow">
        {{ t('update.install') }}
      </el-button>
      <el-button v-if="phase === 'ready'" type="primary" size="small" @click="updater.restartNow">
        {{ t('update.restart') }}
      </el-button>
      <el-button v-if="phase === 'error' && pending" size="small" @click="updater.installNow">
        {{ t('update.retry') }}
      </el-button>
    </div>

    <el-progress
      v-if="phase === 'downloading'"
      class="progress"
      :percentage="percent ?? 0"
      :indeterminate="percent === null"
      :show-text="percent !== null"
      :stroke-width="8"
    />

    <!-- 更新说明原样展示（纯文本，不渲染 Markdown），保留服务端换行 -->
    <div v-if="pending && phase !== 'latest'" class="notes">
      <div class="notes-head">
        <span>{{ t('update.notes') }}</span>
        <span class="dim">{{ t('update.published').replace('{date}', fmtDate(pending.date)) }}</span>
      </div>
      <p v-if="pending.notes" class="notes-body">{{ pending.notes }}</p>
      <p v-else class="dim notes-body">{{ t('update.noNotes') }}</p>
    </div>

    <p v-if="error" class="err">{{ error }}</p>

    <div class="auto-row">
      <el-switch v-model="autoCheckModel" size="small" />
      <span>{{ t('update.auto') }}</span>
    </div>
    <p class="dim hint">{{ t('update.autoHint') }}</p>
    <p class="dim hint">{{ t('update.offline') }}</p>
  </div>
</template>

<style scoped>
.desc {
  margin-top: 10px;
  font-size: 12px;
  line-height: 1.7;
}
.status {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  flex-wrap: wrap;
  font-size: 12px;
}
.badge {
  display: inline-block;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
}
.badge.ok {
  color: var(--ok);
  background: rgba(79, 240, 122, 0.12);
}
.badge.muted {
  color: var(--text-dim);
  background: var(--bg-card);
  border: 1px solid var(--border);
}
.badge.err {
  color: var(--critical);
  background: rgba(255, 92, 92, 0.12);
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
  flex-wrap: wrap;
}
.progress {
  margin-top: 12px;
  max-width: 420px;
}
.notes {
  margin-top: 14px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-card);
  font-size: 12px;
}
.notes-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-weight: 600;
}
.notes-body {
  margin-top: 8px;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.7;
}
.err {
  margin-top: 10px;
  font-size: 12px;
  color: var(--critical);
  word-break: break-all;
}
.auto-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  font-size: 12px;
}
.hint {
  margin-top: 8px;
  font-size: 11px;
  line-height: 1.7;
}
</style>
