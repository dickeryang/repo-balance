<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useScanStore } from '../../stores/scan'
import { useI18n } from '../../i18n'
import * as ipc from '../../ipc/commands'
import type { Category, CustomRule, Severity } from '../../types/models'

const store = useScanStore()
const { t } = useI18n()

const SEVERITIES: Severity[] = ['critical', 'warning', 'info']
const CATEGORIES: Category[] = ['structure', 'history', 'branches', 'deps', 'security']

/**
 * 本地可编辑副本：把 store 配置深拷贝一份供表单直接 v-model，
 * 变更经 deep watch 回写 store（setScanConfig 负责持久化到 localStorage）。
 * 直接改 store 里的对象会绕过持久化，故不采用。
 */
function clone(configs: CustomRule[]): CustomRule[] {
  return JSON.parse(JSON.stringify(configs)) as CustomRule[]
}

const rules = ref<CustomRule[]>(clone(store.scanConfig.customRules))

watch(
  rules,
  (next) => {
    store.setScanConfig({ ...store.scanConfig, customRules: clone(next) })
  },
  { deep: true },
)

/** 规则 ID 在列表中出现的次数（重复检测）。 */
function idCount(id: string): number {
  const trimmed = id.trim()
  return rules.value.filter((r) => r.id.trim() === trimmed).length
}

/**
 * 客户端校验（与 rb-core 的 CustomRule::validate 同一口径）：
 * 后端还会兜底校验并拒绝扫描，这里是为了即时反馈。
 */
function validate(rule: CustomRule): string | null {
  if (!rule.id.trim()) return t('rules.errId')
  if (!/^[A-Za-z0-9._-]+$/.test(rule.id.trim())) return t('rules.errIdChars')
  if (idCount(rule.id) > 1) return t('rules.errDuplicate')
  if (!rule.title.trim()) return t('rules.errTitle')
  if (!rule.pattern.trim()) return t('rules.errPattern')
  try {
    new RegExp(rule.pattern, rule.ignoreCase ? 'i' : '')
  } catch {
    return t('rules.errRegex')
  }
  if (!rule.suggestion.trim()) return t('rules.errSuggestion')
  return null
}

const errors = computed<(string | null)[]>(() => rules.value.map(validate))
const hasError = computed(() => errors.value.some((e) => e !== null))

function addRule() {
  rules.value = [
    ...rules.value,
    {
      id: '',
      title: '',
      severity: 'warning',
      category: 'structure',
      pattern: '',
      pathGlob: null,
      ignoreCase: false,
      suggestion: '',
    },
  ]
}

function removeRule(index: number) {
  rules.value = rules.value.filter((_, i) => i !== index)
}

async function importRules() {
  try {
    const imported = await ipc.importCustomRules()
    if (imported === null) return // 用户取消
    rules.value = clone(imported)
    ElMessage.success(t('rules.imported').replace('{n}', String(imported.length)))
  } catch (e) {
    ElMessage.error(`${t('rules.importFailed')}: ${String(e)}`)
  }
}

async function exportRules() {
  // 导出前先本地校验：不让非法规则进文件（后端另有同口径兜底校验）。
  if (hasError.value) {
    ElMessage.error(t('rules.exportBlocked'))
    return
  }
  try {
    const path = await ipc.exportCustomRules(rules.value)
    if (path) ElMessage.success(t('rules.exported').replace('{path}', path))
  } catch (e) {
    ElMessage.error(`${t('rules.exportFailed')}: ${String(e)}`)
  }
}
</script>

<template>
  <div class="rules-editor">
    <div class="rules-head">
      <span class="config-subtitle">{{ t('rules.title') }}</span>
      <span class="dim">{{ rules.length }} {{ t('rules.count') }}</span>
      <button class="btn" @click="addRule">{{ t('rules.add') }}</button>
      <button class="btn" @click="importRules">{{ t('rules.import') }}</button>
      <button class="btn" :disabled="rules.length === 0" @click="exportRules">
        {{ t('rules.export') }}
      </button>
    </div>
    <p class="dim hint">{{ t('rules.desc') }}</p>
    <p v-if="hasError" class="invalid">{{ t('rules.invalid') }}</p>
    <p v-else-if="rules.length === 0" class="dim hint">{{ t('rules.empty') }}</p>

    <div v-for="(rule, i) in rules" :key="i" class="rule-card">
      <div class="rule-row">
        <label :for="`rule-id-${i}`">{{ t('rules.id') }}</label>
        <input
          :id="`rule-id-${i}`"
          v-model="rule.id"
          class="mono"
          :placeholder="t('rules.idHint')"
        />
        <label :for="`rule-title-${i}`">{{ t('rules.name') }}</label>
        <input :id="`rule-title-${i}`" v-model="rule.title" class="grow" />
        <button class="btn danger" :title="t('rules.remove')" @click="removeRule(i)">✕</button>
      </div>
      <div class="rule-row">
        <label :for="`rule-pattern-${i}`">{{ t('rules.pattern') }}</label>
        <input
          :id="`rule-pattern-${i}`"
          v-model="rule.pattern"
          class="mono grow"
          placeholder="console\.log"
        />
      </div>
      <div class="rule-row">
        <label :for="`rule-path-${i}`">{{ t('rules.pathGlob') }}</label>
        <!-- 留空 = 不限路径：空串统一归一化为 null（与后端 DTO 口径一致）。 -->
        <input
          :id="`rule-path-${i}`"
          :value="rule.pathGlob ?? ''"
          class="mono"
          :placeholder="t('rules.pathGlobHint')"
          @input="rule.pathGlob = ($event.target as HTMLInputElement).value || null"
        />
        <label :for="`rule-sev-${i}`">{{ t('rules.severity') }}</label>
        <select :id="`rule-sev-${i}`" v-model="rule.severity">
          <option v-for="s in SEVERITIES" :key="s" :value="s">{{ t(`filter.${s}`) }}</option>
        </select>
        <label :for="`rule-cat-${i}`">{{ t('rules.category') }}</label>
        <select :id="`rule-cat-${i}`" v-model="rule.category">
          <option v-for="c in CATEGORIES" :key="c" :value="c">{{ t(`filter.${c}`) }}</option>
        </select>
        <label class="toggle">
          <input type="checkbox" v-model="rule.ignoreCase" />
          <span>{{ t('rules.ignoreCase') }}</span>
        </label>
      </div>
      <div class="rule-row">
        <label :for="`rule-sug-${i}`">{{ t('rules.suggestion') }}</label>
        <input :id="`rule-sug-${i}`" v-model="rule.suggestion" class="grow" />
      </div>
      <p v-if="errors[i]" class="err">{{ errors[i] }}</p>
    </div>
  </div>
</template>

<style scoped>
.rules-editor {
  margin-top: 20px;
}
.rules-head {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}
.rules-head .config-subtitle {
  border-top: none;
  padding-top: 0;
  margin-top: 0;
}
.rules-head .dim {
  font-size: 11px;
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
.btn.danger:hover {
  border-color: var(--critical);
  color: var(--critical);
}
.rule-card {
  margin-top: 12px;
  padding: 12px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
}
.rule-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}
.rule-row:last-child {
  margin-bottom: 0;
}
.rule-row label {
  font-size: 12px;
  color: var(--text-dim);
}
.rule-row input.mono,
.rule-row input.grow {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 5px 9px;
  color: var(--text);
  font-size: 12px;
}
.rule-row input:focus,
.rule-row select:focus {
  outline: none;
  border-color: var(--accent);
}
.rule-row input.mono {
  font-family: 'SF Mono', Menlo, monospace;
  width: 190px;
}
.rule-row input.grow {
  flex: 1;
  min-width: 160px;
}
.rule-row select {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 5px 6px;
  color: var(--text);
  font-size: 12px;
}
.toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  font-size: 12px;
  color: var(--text-dim);
}
.toggle input {
  width: 15px;
  height: 15px;
  accent-color: var(--accent);
}
.err {
  margin-top: 6px;
  font-size: 11px;
  color: var(--critical);
}
.invalid {
  margin-top: 10px;
  font-size: 11px;
  color: var(--critical);
}
.config-subtitle {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-dim);
}
.hint {
  margin-top: 8px;
  font-size: 11px;
}
.dim {
  color: var(--text-dim);
}
</style>
