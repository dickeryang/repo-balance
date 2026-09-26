import { invoke } from '@tauri-apps/api/core'
import type { RepoSnapshot, ScanReport } from '../types/models'

export async function selectDirectory(): Promise<string | null> {
  return invoke<string | null>('select_directory')
}

export async function repoInfo(path: string): Promise<RepoSnapshot> {
  return invoke<RepoSnapshot>('repo_info', { path })
}

export async function startScan(path: string): Promise<void> {
  return invoke<void>('start_scan', { path })
}

export async function cancelScan(): Promise<void> {
  return invoke<void>('cancel_scan')
}

export async function exportReport(format: string, report: ScanReport): Promise<string | null> {
  return invoke<string | null>('export_report', { format, report })
}