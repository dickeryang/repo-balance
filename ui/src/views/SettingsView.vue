<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import * as ipc from '../ipc/commands'
import type { VulnIndexInfo } from '../types/models'
import ConfigPanel from './connect/ConfigPanel.vue'
import IgnoreRulesEditor from './settings/IgnoreRulesEditor.vue'
import UpdatePanel from '../components/UpdatePanel.vue'

const store = useScanStore()
const { t } = useI18n()

/** 离线漏洞库状态；null = 未载入（依赖维度只做结构检查）。 */
const vulnInfo = ref<VulnIndexInfo | null>(null)

async function refreshVulnInfo() {
  try {
    vulnInfo.value = await ipc.vulnIndexInfo()
  } catch (e) {
    // 状态查询失败不影响设置页其它配置，保持「未载入」展示即可
    console.error('读取漏洞库状态失败:', e)
    vulnInfo.value = null
  }
}

/** 导入索引：先验后写（后端同样校验），成功后刷新状态。 */
async function importVulnIndex() {
  try {
    const count = await ipc.importVulnIndex()
    if (count === null) return
    ElMessage.success(t('vuln.imported').replace('{n}', String(count)))
    await refreshVulnInfo()
  } catch (e) {
    ElMessage.error(`${t('vuln.importFailed')}: ${String(e)}`)
  }
}

/** 更新时间展示：Unix 秒 → 年-月-日（无 mtime 时显示「—」）。 */
function fmtUpdatedAt(seconds: number | null): string {
  if (!seconds) return '—'
  const d = new Date(seconds * 1000)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

function fmtMiB(bytes: number): string {
  return `${(bytes / 1_048_576).toFixed(1)} MiB`
}

onMounted(refreshVulnInfo)
</script>

<template>
  <div class="card settings-head">
    <div class="head">
      <h2>{{ t('settings.title') }}</h2>
      <!-- 扫描进行中时返回进度页，否则回接入页（避免「进了设置回不到进度」） -->
      <el-button size="small" @click="store.setCurrentView(store.isScanning ? 2 : 1)">
        {{ t('settings.back') }}
      </el-button>
    </div>
    <p class="dim desc">{{ t('settings.desc') }}</p>
  </div>

  <!-- 配置项全局生效，与是否已选仓库无关（进入扫描前改也来得及）。 -->
  <ConfigPanel />

  <!-- 忽略规则按仓库存放（.repobalance-ignore），故编辑器需要选定目标仓库。 -->
  <div class="card">
    <IgnoreRulesEditor />
  </div>

  <div class="card">
    <h2>{{ t('vuln.title') }}</h2>
    <div class="vuln-status">
      <template v-if="vulnInfo">
        <span class="badge ok">
          {{ t('vuln.loaded').replace('{n}', String(vulnInfo.entries)) }}
        </span>
        <span class="dim">
          {{
            t('vuln.updated')
              .replace('{date}', fmtUpdatedAt(vulnInfo.updatedAt))
              .replace('{size}', fmtMiB(vulnInfo.bytes))
          }}
        </span>
      </template>
      <span v-else class="badge muted">{{ t('vuln.missing') }}</span>
    </div>
    <div class="vuln-actions">
      <el-button size="small" @click="importVulnIndex">{{ t('vuln.import') }}</el-button>
      <el-button size="small" plain @click="refreshVulnInfo">{{ t('vuln.refresh') }}</el-button>
    </div>
    <p class="dim hint">{{ t('vuln.hint') }}</p>
  </div>

  <!-- 唯一联网入口：默认不发起任何请求，仅在用户显式操作时联网 -->
  <UpdatePanel />
</template>

<style scoped>
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.desc {
  margin-top: 10px;
  font-size: 12px;
  line-height: 1.7;
}
.vuln-status {
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
.vuln-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}
.hint {
  margin-top: 12px;
  font-size: 11px;
  line-height: 1.7;
}
</style>
