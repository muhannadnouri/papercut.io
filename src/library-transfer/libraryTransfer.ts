import { invoke, isTauri } from '@tauri-apps/api/core'

export interface LibraryTransferExportResult {
  documents: number
  audiobooks: number
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
  audiobooksSelected: number
  audiobooksImported: number
  audiobooksSkipped: number
  audiobooksFailed: number
  importedAudiobooks: ImportedAudiobookRecord[]
  failures: LibraryTransferFailure[]
}

export interface ImportedAudiobookRecord {
  id: string
  documentUrl: string
  title: string
  voice: string
  speed: number
  modelId: string
  textPreprocessor: string
  silmaNfeStep?: number
  dtype: string
  savedAt: number
  chunks: number
  audioDurationSec: number
  wavBytes: number
}

export async function exportLibrary(includeAudiobooks = false): Promise<LibraryTransferExportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferExportResult | null>('library_transfer_export', {
    request: { includeAudiobooks },
  })
}

export async function importLibrary(): Promise<LibraryTransferImportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferImportResult | null>('library_transfer_import')
}
