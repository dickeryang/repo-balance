/**
 * 忽略规则编辑器的纯逻辑（与 Vue 渲染分离，便于单元测试）。
 *
 * 只放「输入 → 输出」确定的小函数：界面负责调用，测试直接覆盖，
 * 不必为这些口径引入 DOM 环境。
 */

/**
 * 被忽略文件占比（一位小数百分比）。
 *
 * 空仓库 / 无文件时返回 0 而不是 NaN：那时「0% 被忽略」与「没有文件」都成立，
 * 界面另有文案说明文件总数。
 */
export function ignoredPercent(ignored: number, total: number): number {
  if (total <= 0) return 0
  return Math.round((ignored / total) * 1000) / 10
}

/** 字节数格式化（GiB / MiB / KiB / B，口径同规模预估卡片）。 */
export function fmtIgnoreBytes(bytes: number): string {
  const mib = bytes / 1_048_576
  if (mib >= 1024) return `${(mib / 1024).toFixed(1)} GiB`
  if (mib >= 1) return `${mib.toFixed(1)} MiB`
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KiB`
  return `${Math.max(0, Math.round(bytes))} B`
}

/** 编辑器内容是否已改动（与「已保存到仓库的内容」逐字比较）。 */
export function isIgnoreDirty(content: string, saved: string): boolean {
  return content !== saved
}

/** 用户规则条数（跳过空行与 `#` 注释）；CRLF 的 `\r` 由 trim 兜住。 */
export function userRuleCount(text: string): number {
  return text.split('\n').filter((line) => {
    const t = line.trim()
    return t.length > 0 && !t.startsWith('#')
  }).length
}
