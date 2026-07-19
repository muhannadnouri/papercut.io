import { invoke, isTauri } from '@tauri-apps/api/core'

export interface LibraryTransferExportResult {
  documents: number
}

export interface LibraryTransferFailure {
  item: string
  error: string
}

export interface LibraryTransferImportResult {
  selected: number
  imported: number
  skipped: number
  failed: number
  foldersCreated: number
  failures: LibraryTransferFailure[]
}

export async function exportLibrary(): Promise<LibraryTransferExportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferExportResult | null>('library_transfer_export')
}

export async function importLibrary(): Promise<LibraryTransferImportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferImportResult | null>('library_transfer_import')
}
