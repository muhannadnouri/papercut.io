import type { UploadedDocumentBatchResult } from '../uploads/DocumentUploads'

export interface DocumentScannerAvailability {
  supported: boolean
  photoImportSupported: boolean
  platform: string
  reason?: string | null
}

export async function getDocumentScannerAvailability(): Promise<DocumentScannerAvailability> {
  if (!isTauriRuntime()) {
    return {
      supported: false,
      photoImportSupported: false,
      platform: 'browser',
      reason: 'Native capture requires the app',
    }
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<DocumentScannerAvailability>('document_scanner_availability')
}

/** Capture stays behind one command so native file paths and page bytes never
 * enter React; callers receive the established document-import result only. */
export async function scanDocument(): Promise<UploadedDocumentBatchResult> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<UploadedDocumentBatchResult>('document_scanner_scan')
}

/** Existing photos follow the same staged PDF import boundary as camera scans. */
export async function importDocumentPhotos(): Promise<UploadedDocumentBatchResult> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<UploadedDocumentBatchResult>('document_scanner_import_images')
}

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}
