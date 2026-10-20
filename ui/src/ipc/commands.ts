import { invoke } from '@tauri-apps/api/core'
import type { CustomRule, RepoSnapshot, ScanReport, ScanConfig, ScaleEstimate } from '../types/models'

/** 批量选择仓库目录（多选）；用户取消返回空数组。 */
export async function selectDirectories(): Promise<string[]> {
  return invoke<string[]>('select_directories')
}

export async function selectDirectory(): Promise<string | null> {
  return invoke<string | null>('select_directory')
}

export async function repoInfo(path: string): Promise<RepoSnapshot> {
  return invoke<RepoSnapshot>('repo_info', { path })
}

export async function scanEstimate(path: string): Promise<ScaleEstimate> {
  return invoke<ScaleEstimate>('scan_estimate', { path })
}

export async function startScan(path: string, config: ScanConfig): Promise<void> {
  return invoke<void>('start_scan', { path, config })
}

/** 远程仓库体检：URL 浅克隆到临时目录后完整扫描（后端自动清理）。 */
export async function startRemoteScan(url: string, depth: number, config: ScanConfig): Promise<void> {
  return invoke<void>('start_remote_scan', { url, depth, config })
}

export async function cancelScan(): Promise<void> {
  return invoke<void>('cancel_scan')
}

export async function exportReport(format: string, report: ScanReport): Promise<string | null> {
  return invoke<string | null>('export_report', { format, report })
}

/** 读取扫描历史（应用数据目录 JSON 文件的原始内容；不存在时为 `[]`）。 */
export async function loadScanHistory(): Promise<string> {
  return invoke<string>('load_scan_history')
}

/** 覆盖写入扫描历史（JSON 数组字符串）。 */
export async function saveScanHistory(data: string): Promise<void> {
  return invoke<void>('save_scan_history', { data })
}

/** 导入扫描历史：弹文件对话框，返回文件内容（取消时为 null）。 */
export async function importScanHistory(): Promise<string | null> {
  return invoke<string | null>('import_scan_history')
}

/** 导出扫描历史：弹保存对话框并写盘，返回实际路径（取消时为 null）。 */
export async function exportScanHistory(data: string): Promise<string | null> {
  return invoke<string | null>('export_scan_history', { data })
}

/** 导入自定义规则：弹文件对话框，返回已校验的规则列表（取消时为 null）。 */
export async function importCustomRules(): Promise<CustomRule[] | null> {
  return invoke<CustomRule[] | null>('import_custom_rules')
}

/** 导出自定义规则：弹保存对话框并写盘，返回实际路径（取消时为 null）。 */
export async function exportCustomRules(rules: CustomRule[]): Promise<string | null> {
  return invoke<string | null>('export_custom_rules', { rules })
}