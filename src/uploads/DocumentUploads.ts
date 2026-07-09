export interface UploadedDocument {
  id: string
  url: string
  title: string
  format: 'html' | string
  importedAtMs: number
  bytes: number
  sections: number
}

export interface UploadedDocumentSearchResult {
  id: string
  documentId: string
  url: string
  title: string
  excerpt: string
  sectionTitle?: string | null
  sectionIndex: number
  matchScope?: 'section' | 'document'
}

export interface UploadedDocumentDeleteResult {
  id: string
  url: string
  bytesFreed: number
}

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
  return /^\/uploads\/[a-fA-F0-9]+\.html(?:[#?].*)?$/.test(url)
}

export async function importHtmlDocument(): Promise<UploadedDocument> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocument>('document_uploads_import_html')
}

export async function importEpubDocument(): Promise<UploadedDocument> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocument>('document_uploads_import_epub')
}

export async function listUploadedDocuments(): Promise<UploadedDocument[]> {
  if (!isTauriRuntime()) return []
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocument[]>('document_uploads_list')
}

export async function searchUploadedDocuments(query: string, limit = 50, documentUrls?: string[]): Promise<UploadedDocumentSearchResult[]> {
  if (!isTauriRuntime() || query.trim().length === 0) return []
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentSearchResult[]>('document_uploads_search', {
    request: { query, limit, documentUrls },
  })
}

export async function getUploadedDocumentSource(documentUrl: string): Promise<string> {
  const invoke = await loadTauriInvoke()
  return invoke<string>('document_uploads_get_source', {
    request: { documentUrl },
  })
}

export async function deleteUploadedDocument(documentUrl: string): Promise<UploadedDocumentDeleteResult> {
  const invoke = await loadTauriInvoke()
  return invoke<UploadedDocumentDeleteResult>('document_uploads_delete', {
    request: { documentUrl },
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
