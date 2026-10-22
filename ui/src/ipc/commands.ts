import { invoke } from '@tauri-apps/api/core'
import type {
  CustomRule,
  IgnoreCheck,
  RepoSnapshot,
  ScanReport,
  ScanConfig,
  ScaleEstimate,
  VulnIndexInfo,
} from '../types/models'

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

/** 导入离线漏洞库索引（JSONL）；返回公告条数，取消时为 null。 */
export async function importVulnIndex(): Promise<number | null> {
  return invoke<number | null>('import_vuln_index')
}

/** 查询离线漏洞库状态；未载入时返回 null。 */
export async function vulnIndexInfo(): Promise<VulnIndexInfo | null> {
  return invoke<VulnIndexInfo | null>('vuln_index_info')
}

/** 导入自定义规则：弹文件对话框，返回已校验的规则列表（取消时为 null）。 */
export async function importCustomRules(): Promise<CustomRule[] | null> {
  return invoke<CustomRule[] | null>('import_custom_rules')
}

/** 导出自定义规则：弹保存对话框并写盘，返回实际路径（取消时为 null）。 */
export async function exportCustomRules(rules: CustomRule[]): Promise<string | null> {
  return invoke<string | null>('export_custom_rules', { rules })
}

/** 读取仓库根目录的 `.repobalance-ignore`；文件不存在返回 null。 */
export async function readIgnoreRules(path: string): Promise<string | null> {
  return invoke<string | null>('read_ignore_rules', { path })
}

/**
 * 保存忽略规则到仓库根目录（原子写）。
 *
 * 这是应用**唯一写入被检查仓库**的操作：忽略规则必须随仓库走（与 .gitignore 同理），
 * 否则换台机器就失效。保存只写这一个文件，不触碰 git 对象库与工作区其它内容。
 */
export async function writeIgnoreRules(path: string, content: string): Promise<void> {
  return invoke<void>('write_ignore_rules', { path, content })
}

/** 忽略规则试算：不落盘，返回影响面统计与静态检查结论。 */
export async function checkIgnoreRules(path: string, content: string): Promise<IgnoreCheck> {
  return invoke<IgnoreCheck>('check_ignore_rules', { path, content })
}

/** 内置默认忽略模式（构建产物 / 依赖目录），只读展示。 */
export async function ignoreDefaults(): Promise<string[]> {
  return invoke<string[]>('ignore_defaults')
}