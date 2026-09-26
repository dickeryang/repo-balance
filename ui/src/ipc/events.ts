import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ScanReport, ProgressEvent } from '../types/models'

export async function onScanDone(callback: (report: ScanReport) => void): Promise<UnlistenFn> {
  return listen<ScanReport>('scan-done', (event) => {
    callback(event.payload)
  })
}

export async function onScanProgress(callback: (progress: ProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ProgressEvent>('scan-progress', (event) => {
    callback(event.payload)
  })
}

export async function onScanCancelled(callback: (error: string) => void): Promise<UnlistenFn> {
  return listen<string>('scan-cancelled', (event) => {
    callback(event.payload)
  })
}