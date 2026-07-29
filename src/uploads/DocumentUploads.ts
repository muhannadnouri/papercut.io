export interface UploadedDocument {
  id: string
  url: string
  title: string
  format: 'html' | string
  sourceKind: 'html' | 'pdf'
  importedAtMs: number
  bytes: number
  sections: number
  coverMediaType?: string | null
}

export interface UploadedDocumentSearchResult {
  id: string
  documentId: string
  url: string
  title: string
  excerpt: string
  sectionTitle?: string | null
  sectionIndex: number
  pageIndex?: number | null
  matchScope?: 'section' | 'document'
}

export interface UploadedDocumentDeleteResult {
  id: string
  url: string
  bytesFreed: number
}

export interface UploadedDocumentDeleteBatchFailure {
  documentUrl: string
  error: string
}

export interface UploadedDocumentDeleteBatchProgress {
  phase: 'deleting' | 'completed'
  processed: number
  total: number
  deleted: number
  failed: number
  documentUrl?: string | null
}

export interface UploadedDocumentDeleteBatchResult {
  selected: number
  processed: number
  deleted: UploadedDocumentDeleteResult[]
  failures: UploadedDocumentDeleteBatchFailure[]
  bytesFreed: number
}

export interface UploadedDocumentBatchFailure {
  fileName: string
  error: string
}

export interface UploadedDocumentBatchProgress {
  phase: 'importing' | 'completed' | 'cancelled'
  processed: number
  total: number
  imported: number
  failed: number
  fileName?: string | null
}

export interface UploadedDocumentBatchResult {
  selected: number
  processed: number
  imported: UploadedDocument[]
  failures: UploadedDocumentBatchFailure[]
  cancelled: boolean
}

export interface PdfPageTextLayer {
  schemaVersion: 1
  pageIndex: number
  width: number
  height: number
  blocks: Array<{
    text: string
    bounds: [number, number, number, number]
    order: number
    confidence: number | null
  }>
}

export interface PdfNarrationSegment {
  text: string
  sourceRuns: Array<{
    pageIndex: number
    blockOrder: number
    startOffset: number
    endOffset: number
    sourceStartOffset: number
    sourceEndOffset: number
  }>
}

const DOCUMENT_IMPORT_PROGRESS_EVENT = 'document-uploads-import-progress'
const DOCUMENT_DELETE_PROGRESS_EVENT = 'document-uploads-delete-progress'

export interface UploadedLibraryFolder {
  id: string
  parentId?: string | null
  name: string
  depth: number
  sortOrder: number
  createdAtMs: number
  updatedAtMs: number
}

export interface UploadedDocumentLocation {
  documentId: string
  folderId?: string | null
  sortOrder: number
}

export interface UploadedLibraryOrganization {
  folders: UploadedLibraryFolder[]
  documentLocations: UploadedDocumentLocation[]
}

export interface UploadedLibraryOrderItem {
  itemType: 'folder' | 'document'
  id: string
}

export function isUploadedDocumentUrl(url: string): boolean {
  return /^\/uploads\/[a-fA-F0-9]+\.(?:html|pdf)(?:[#?].*)?$/.test(url)
}

export function isUploadedHtmlDocumentUrl(url: string): boolean {
  return /^\/uploads\/[a-fA-F0-9]+\.html(?:[#?].*)?$/.test(url)
}

export function isUploadedPdfDocumentUrl(url: string): boolean {
  return /^\/uploads\/[a-fA-F0-9]+\.pdf(?:[#?].*)?$/.test(url)
}

export async function importDocumentBatch(): Promise<UploadedDocumentBatchResult> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentBatchResult>('document_uploads_import_batch')
}

export async function importDocumentFolder(): Promise<UploadedDocumentBatchResult> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentBatchResult>('document_uploads_import_folder')
}

export async function cancelDocumentBatch(): Promise<boolean> {
  const invoke = await loadTauriInvoke()
  return invoke<boolean>('document_uploads_cancel_import_batch')
}

export async function listenDocumentBatchProgress(
  handler: (progress: UploadedDocumentBatchProgress) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<UploadedDocumentBatchProgress>(DOCUMENT_IMPORT_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

export async function listUploadedDocuments(): Promise<UploadedDocument[]> {
  if (!isTauriRuntime()) return []
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocument[]>('document_uploads_list')
}

export async function searchUploadedDocuments(
  query: string,
  limit = 50,
  documentUrls?: string[],
  exactPhrases?: string[],
): Promise<UploadedDocumentSearchResult[]> {
  if (!isTauriRuntime() || query.trim().length === 0) return []
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentSearchResult[]>('document_uploads_search', {
    request: { query, limit, documentUrls, exactPhrases },
  })
}

export async function getUploadedDocumentSource(documentUrl: string): Promise<string> {
  const invoke = await loadTauriInvoke()
  return invoke<string>('document_uploads_get_source', {
    request: { documentUrl },
  })
}

export async function getUploadedDocumentCover(documentUrl: string): Promise<string | null> {
  if (!isTauriRuntime()) return null
  const invoke = await loadTauriInvoke()
  return invoke<string | null>('document_uploads_get_cover', {
    request: { documentUrl },
  })
}

export async function getUploadedPdfSource(documentUrl: string): Promise<Uint8Array> {
  const invoke = await loadTauriInvoke()
  const source = await invoke<ArrayBuffer | Uint8Array | number[]>('document_uploads_get_pdf_source', {
    request: { documentUrl },
  })
  if (source instanceof Uint8Array) return source
  return new Uint8Array(source)
}

export async function getUploadedPdfAssetUrl(documentUrl: string): Promise<string> {
  const mod = await import('@tauri-apps/api/core')
  const path = await mod.invoke<string>('document_uploads_get_pdf_asset_path', {
    request: { documentUrl },
  })
  return mod.convertFileSrc(path)
}

export async function getUploadedPdfNarrationSegments(
  documentUrl: string,
): Promise<PdfNarrationSegment[]> {
  const invoke = await loadTauriInvoke()
  return invoke<PdfNarrationSegment[]>('document_uploads_get_pdf_narration_segments', {
    request: { documentUrl },
  })
}

export async function storeUploadedPdfPageText(
  documentUrl: string,
  layer: PdfPageTextLayer,
): Promise<void> {
  const invoke = await loadTauriInvoke()
  await invoke<void>('document_uploads_store_pdf_page_text', {
    request: { documentUrl, layer },
  })
}

export async function finalizeUploadedPdf(
  documentUrl: string,
  title: string | undefined,
  pageCount: number,
  thumbnail?: number[],
): Promise<UploadedDocument> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocument>('document_uploads_finalize_pdf', {
    request: { documentUrl, title, pageCount, thumbnail },
  })
}

export async function deleteUploadedDocument(documentUrl: string): Promise<UploadedDocumentDeleteResult> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentDeleteResult>('document_uploads_delete', {
    request: { documentUrl },
  })
}

export async function deleteUploadedDocuments(documentUrls: string[]): Promise<UploadedDocumentDeleteBatchResult> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentDeleteBatchResult>('document_uploads_delete_batch', {
    request: { documentUrls },
  })
}

export async function listenDocumentDeleteProgress(
  handler: (progress: UploadedDocumentDeleteBatchProgress) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<UploadedDocumentDeleteBatchProgress>(DOCUMENT_DELETE_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

export async function getUploadedLibraryOrganization(): Promise<UploadedLibraryOrganization> {
  if (!isTauriRuntime()) return { folders: [], documentLocations: [] }
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryOrganization>('document_uploads_library_organization')
}

export async function createUploadedLibraryFolder(parentId: string | null, name: string): Promise<UploadedLibraryFolder> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryFolder>('document_uploads_create_folder', {
    request: { parentId, name },
  })
}

export async function renameUploadedLibraryFolder(folderId: string, name: string): Promise<UploadedLibraryFolder> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryFolder>('document_uploads_rename_folder', {
    request: { folderId, name },
  })
}

export async function deleteUploadedLibraryFolder(folderId: string): Promise<void> {
  const invoke = await loadTauriInvoke()
  await invoke<void>('document_uploads_delete_folder', {
    request: { folderId },
  })
}

export async function moveUploadedDocuments(documentIds: string[], folderId: string | null): Promise<UploadedLibraryOrganization> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryOrganization>('document_uploads_move_documents', {
    request: { documentIds, folderId },
  })
}

export async function moveUploadedLibraryFolder(folderId: string, parentId: string | null): Promise<UploadedLibraryOrganization> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryOrganization>('document_uploads_move_folder', {
    request: { folderId, parentId },
  })
}

export async function reorderUploadedLibrary(parentId: string | null, items: UploadedLibraryOrderItem[]): Promise<UploadedLibraryOrganization> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedLibraryOrganization>('document_uploads_reorder_library', {
    request: { parentId, items },
  })
}

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

async function loadTauriInvoke(): Promise<<T>(cmd: string, args?: Record<string, unknown>) => Promise<T>> {
  const mod = await import('@tauri-apps/api/core')
  return mod.invoke
}
