<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import * as ipc from '../../ipc/commands'
import type { IgnoreCheck, IgnoreLint } from '../../types/models'
import {
  fmtIgnoreBytes,
  ignoredPercent,
  isIgnoreDirty,
  userRuleCount,
} from '../../utils/ignore'

const store = useScanStore()
const { t } = useI18n()

/** 编辑器当前内容（可含未保存修改）。 */
const content = ref('')
/** 仓库中已落盘的内容；`null` = 文件尚不存在。 */
const saved = ref<string | null>(null)
/** 内置默认忽略模式（只读展示，让用户知道哪些目录默认就被忽略）。 */
const defaults = ref<string[]>([])
/** 最近一次试算结果（不落盘求值）。 */
const check = ref<IgnoreCheck | null>(null)
/** 进行中的操作；同时用于禁用按钮防重复提交。 */
const busy = ref<'check' | 'save' | null>(null)
const loadError = ref<string | null>(null)

/**
 * 编辑器正在编辑的仓库。
 *
 * 与 `store.selectedPath` 刻意分开：有未保存修改时不跟随切换到别的仓库——
 * 否则一段还没写完的规则会被静默丢掉（更糟的是被保存进另一个仓库）。
 */
const targetPath = ref<string | null>(null)
const dirty = computed(() => isIgnoreDirty(content.value, saved.value ?? ''))
const ruleCount = computed(() => userRuleCount(content.value))
const percent = computed(() =>
  check.value ? ignoredPercent(check.value.ignoredCount, check.value.totalFiles) : 0,
)

/** 读取目标仓库的现有规则；文件不存在时以空内容开始（保存即创建）。 */
async function load() {
  loadError.value = null
  check.value = null
  const path = targetPath.value
  if (!path) {
    content.value = ''
    saved.value = null
    return
  }
  try {
    const text = await ipc.readIgnoreRules(path)
    saved.value = text
    content.value = text ?? ''
  } catch (e) {
    loadError.value = String(e)
    saved.value = null
    content.value = ''
  }
}

/** 有未保存修改时先征得同意，避免一次点击把写了一半的规则丢掉。 */
async function confirmDiscard(): Promise<boolean> {
  if (!dirty.value) return true
  try {
    await ElMessageBox.confirm(t('ignore.discardConfirm'), t('ignore.dirty'), {
      type: 'warning',
      confirmButtonText: t('ignore.discard'),
      cancelButtonText: t('ignore.cancel'),
    })
    return true
  } catch {
    return false
  }
}

/** 重新读取仓库文件（会丢弃未保存修改，故先确认）。 */
async function reload() {
  if (!(await confirmDiscard())) return
  await load()
}

/**
 * 跟随 store 中当前选中的仓库。
 *
 * 无未保存修改时静默跟随（在接入页换了仓库，设置页自然跟着看新仓库的规则）；
 * 有未保存修改时**不跟随**，只提示保留——让用户自己决定何时丢弃。
 */
async function followStoreRepo() {
  const next = store.selectedPath
  if (!next || next === targetPath.value) return
  if (!(await confirmDiscard())) {
    ElMessage.warning(t('ignore.keptEdits'))
    return
  }
  targetPath.value = next
  await load()
}

/** 选择 / 更换目标仓库（复用接入页的目录选择，顺带刷新仓库快照）。 */
async function pickRepo() {
  await store.selectDirectory()
  await followStoreRepo()
}

/**
 * 试算：把编辑器里尚未保存的内容交给后端按扫描口径求值。
 *
 * 这是编辑器的核心价值——「保存前看清规则到底忽略了什么」，
 * 避免「以为忽略了、扫描照旧报警」的静默失效。
 */
async function dryRun() {
  const path = targetPath.value
  if (!path || busy.value) return
  busy.value = 'check'
  try {
    check.value = await ipc.checkIgnoreRules(path, content.value)
  } catch (e) {
    ElMessage.error(`${t('ignore.check')}: ${String(e)}`)
  } finally {
    busy.value = null
  }
}

/** 保存到仓库根目录（唯一写入被检查仓库的操作，按钮文案已如实标注）。 */
async function save() {
  const path = targetPath.value
  if (!path || busy.value) return
  busy.value = 'save'
  try {
    await ipc.writeIgnoreRules(path, content.value)
    saved.value = content.value
    ElMessage.success(t('ignore.saved').replace('{file}', '.repobalance-ignore'))
  } catch (e) {
    ElMessage.error(`${t('ignore.saveFailed')}: ${String(e)}`)
  } finally {
    busy.value = null
  }
}

/** 单条静态检查结论的展示文案（行号 + 模式 + 原因）。 */
function lintText(lint: IgnoreLint): string {
  // 用函数式替换：用户模式里可能含 `$&`、`$1` 等替换语法，不能当模板展开。
  return t('ignore.lintLine')
    .replace('{line}', () => String(lint.line))
    .replace('{pattern}', () => lint.pattern)
    .replace('{why}', () => t(`ignore.lint.${lint.kind}`))
}

// 接入页换了仓库时跟随（有未保存修改则由 followStoreRepo 拦下并提示）。
watch(() => store.selectedPath, followStoreRepo)

onMounted(async () => {
  targetPath.value = store.selectedPath
  await load()
  try {
    defaults.value = await ipc.ignoreDefaults()
  } catch {
    // 默认项只用于展示，取不到就整块不显示，不影响编辑。
    defaults.value = []
  }
})
</script>

<template>
  <div class="ignore-editor">
    <div class="ignore-head">
      <span class="config-subtitle">{{ t('ignore.title') }}</span>
      <span class="dim">{{ ruleCount }} {{ t('ignore.ruleCount') }}</span>
      <span v-if="dirty" class="badge dirty">{{ t('ignore.dirty') }}</span>
    </div>
    <p class="dim hint">{{ t('ignore.desc') }}</p>

    <div class="repo-row">
      <label>{{ t('ignore.target') }}</label>
      <code v-if="targetPath" class="path">{{ targetPath }}</code>
      <span v-else class="dim">{{ t('ignore.noRepo') }}</span>
      <button class="btn" @click="pickRepo">
        {{ targetPath ? t('connect.reselDir') : t('ignore.pickRepo') }}
      </button>
    </div>

    <textarea
      v-model="content"
      class="rules-input mono"
      rows="10"
      spellcheck="false"
      :disabled="!targetPath"
      :placeholder="t('ignore.syntaxHint')"
    ></textarea>

    <div class="actions">
      <button class="btn" :disabled="!targetPath || busy !== null" @click="reload">
        {{ t('ignore.reload') }}
      </button>
      <button class="btn" :disabled="!targetPath || busy !== null" @click="dryRun">
        {{ busy === 'check' ? t('ignore.checking') : t('ignore.check') }}
      </button>
      <button
        class="btn primary"
        :disabled="!targetPath || busy !== null"
        @click="save"
      >
        {{ busy === 'save' ? t('ignore.saving') : t('ignore.save') }}
      </button>
    </div>

    <p v-if="loadError" class="err">{{ t('ignore.readFailed') }}: {{ loadError }}</p>
    <p v-else-if="targetPath" class="dim hint">
      {{ saved === null ? t('ignore.notExists') : t('ignore.exists') }}
    </p>
    <p class="dim hint">{{ t('ignore.writeNotice') }}</p>
    <!-- 有未保存修改时若接入页已切换仓库，明确告知编辑器仍指向旧仓库（保存会写旧仓库）。 -->
    <p v-if="dirty && store.selectedPath && store.selectedPath !== targetPath" class="err">
      {{ t('ignore.keptEdits') }}
    </p>

    <div v-if="check" class="result">
      <p v-if="check.ignoredCount > 0" class="stat">
        {{
          t('ignore.checkResult')
            .replace('{total}', String(check.totalFiles))
            .replace('{ignored}', String(check.ignoredCount))
            .replace('{percent}', String(percent))
            .replace('{size}', fmtIgnoreBytes(check.ignoredBytes))
        }}
      </p>
      <p v-else class="stat dim">{{ t('ignore.noIgnored') }}</p>

      <template v-if="check.samples.length > 0">
        <div class="sub">{{ t('ignore.samplesTitle') }}</div>
        <ul class="samples">
          <li v-for="s in check.samples" :key="s" class="mono">{{ s }}</li>
        </ul>
        <p v-if="check.truncated" class="dim hint">
          {{ t('ignore.truncated').replace('{n}', String(check.samples.length)) }}
        </p>
      </template>

      <div class="sub">{{ t('ignore.lintTitle') }}</div>
      <ul v-if="check.lints.length > 0" class="lints">
        <li v-for="(l, i) in check.lints" :key="`${l.line}-${i}`">{{ lintText(l) }}</li>
      </ul>
      <p v-else class="dim hint">{{ t('ignore.noLint') }}</p>
    </div>

    <div v-if="defaults.length > 0" class="defaults">
      <div class="sub">{{ t('ignore.defaultsTitle').replace('{n}', String(defaults.length)) }}</div>
      <div class="chips">
        <code v-for="d in defaults" :key="d" class="chip mono">{{ d }}</code>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ignore-editor {
  margin-top: 20px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}
.ignore-head {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.ignore-head .dim {
  font-size: 11px;
}
.config-subtitle {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-dim);
}
.badge {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
}
.badge.dirty {
  color: var(--warning);
  background: color-mix(in srgb, var(--warning) 14%, transparent);
}
.repo-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  flex-wrap: wrap;
  font-size: 12px;
}
.repo-row label {
  color: var(--text-dim);
}
.path {
  background: var(--bg-card);
  padding: 3px 8px;
  border-radius: 4px;
  font-family: 'SF Mono', Menlo, monospace;
  font-size: 11px;
  word-break: break-all;
}
.rules-input {
  width: 100%;
  margin-top: 10px;
  padding: 10px 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  color: var(--text);
  font-size: 12px;
  line-height: 1.7;
  resize: vertical;
}
.rules-input:focus {
  outline: none;
  border-color: var(--accent);
}
.rules-input:disabled {
  opacity: 0.5;
}
.mono {
  font-family: 'SF Mono', Menlo, monospace;
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
  flex-wrap: wrap;
}
.btn {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text);
  font-size: 12px;
  padding: 5px 10px;
  cursor: pointer;
}
.btn:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn.primary {
  border-color: var(--accent);
  color: var(--accent);
}
.result,
.defaults {
  margin-top: 14px;
  padding: 12px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
}
.stat {
  font-size: 12px;
}
.sub {
  margin-top: 10px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-dim);
}
.samples,
.lints {
  margin: 6px 0 0;
  padding-left: 18px;
  font-size: 11px;
  line-height: 1.8;
  word-break: break-all;
}
.lints {
  color: var(--warning);
}
.chips {
  display: flex;
  gap: 6px;
  margin-top: 8px;
  flex-wrap: wrap;
}
.chip {
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  font-size: 11px;
}
.hint {
  margin-top: 8px;
  font-size: 11px;
}
.dim {
  color: var(--text-dim);
}
.err {
  margin-top: 8px;
  font-size: 11px;
  color: var(--critical);
}
</style>
