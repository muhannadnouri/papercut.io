import { useCallback, useEffect, useRef, useState } from 'react'
import type { DocumentInfo } from '../types/search'
import {
  cancelDocumentBatch as cancelDocumentBatchSource,
  createUploadedLibraryFolder,
  deleteUploadedDocument,
  deleteUploadedLibraryFolder,
  getUploadedLibraryOrganization,
  importDocumentBatch as importDocumentBatchSource,
  importDocumentFolder as importDocumentFolderSource,
  importEpubDocument as importEpubDocumentSource,
  importHtmlDocument as importHtmlDocumentSource,
  listUploadedDocuments,
  listenDocumentBatchProgress,
  moveUploadedDocuments,
  renameUploadedLibraryFolder,
  type UploadedDocument,
  type UploadedDocumentBatchProgress,
  type UploadedDocumentBatchResult,
  type UploadedLibraryOrganization,
} from '../uploads/DocumentUploads'

type UploadedLibraryState = {
  documents: UploadedDocument[]
  organization: UploadedLibraryOrganization
}

// Keep operation state locale-neutral so the owning UI can translate it and
// isolate user titles without parsing preformatted English messages.
export type DocumentImportStatus = {
  status: 'idle' | 'importing' | 'imported' | 'deleting' | 'deleted' | 'cancelled' | 'error'
  format?: 'html' | 'epub' | 'batch' | 'folder'
  title?: string
  bytesFreed?: number
  message?: string
  batchProgress?: UploadedDocumentBatchProgress
  batchResult?: UploadedDocumentBatchResult
  cancelRequested?: boolean
}

async function loadUploadedLibrary(): Promise<UploadedLibraryState> {
  const [documents, organization] = await Promise.all([
    listUploadedDocuments(),
    getUploadedLibraryOrganization(),
  ])
  return { documents, organization }
}

/**
 * Keeps uploaded document data and library organization in sync with the
 * Tauri upload APIs. App-wide follow-up work, like opening imported documents
 * or clearing search results, stays in App where those dependencies live.
 */
export function useUploadedLibrary() {
  const [uploadedDocuments, setUploadedDocuments] = useState<UploadedDocument[]>([])
  const [uploadedLibraryOrganization, setUploadedLibraryOrganization] = useState<UploadedLibraryOrganization>({ folders: [], documentLocations: [] })
  const [documentImport, setDocumentImport] = useState<DocumentImportStatus>({ status: 'idle' })
  const operationInProgressRef = useRef(false)

  const applyUploadedLibrary = useCallback((library: UploadedLibraryState) => {
    setUploadedDocuments(library.documents)
    setUploadedLibraryOrganization(library.organization)
  }, [])

  const refreshUploadedLibrary = useCallback(async () => {
    applyUploadedLibrary(await loadUploadedLibrary())
  }, [applyUploadedLibrary])

  useEffect(() => {
    let cancelled = false
    loadUploadedLibrary().then((library) => {
      if (!cancelled) applyUploadedLibrary(library)
    }).catch((err) => {
      console.warn('Unable to load uploaded documents:', err)
    })

    return () => {
      cancelled = true
    }
  }, [applyUploadedLibrary])

  const importDocument = useCallback(async (
    format: 'html' | 'epub',
    importer: () => Promise<UploadedDocument>,
  ): Promise<UploadedDocument | null> => {
    if (operationInProgressRef.current) return null
    operationInProgressRef.current = true
    setDocumentImport({ status: 'importing', format })
    try {
      const result = await importer()
      await refreshUploadedLibrary()
      setDocumentImport({ status: 'imported', format, title: result.title })
      return result
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setDocumentImport({
        status: cancelled ? 'cancelled' : 'error',
        format,
        message: cancelled ? undefined : message,
      })
      return null
    } finally {
      operationInProgressRef.current = false
    }
  }, [refreshUploadedLibrary])

  const importHtmlDocument = useCallback(
    () => importDocument('html', importHtmlDocumentSource),
    [importDocument],
  )

  const importEpubDocument = useCallback(
    () => importDocument('epub', importEpubDocumentSource),
    [importDocument],
  )

  /** Subscribe before opening the picker so even the first native progress event
   * is retained; both collection pickers share refresh and partial-result flow. */
  const importDocumentCollection = useCallback(async (
    format: 'batch' | 'folder',
    importer: () => Promise<UploadedDocumentBatchResult>,
  ): Promise<UploadedDocumentBatchResult | null> => {
    if (operationInProgressRef.current) return null
    operationInProgressRef.current = true
    setDocumentImport({ status: 'importing', format })
    let unlisten: (() => void) | undefined
    try {
      unlisten = await listenDocumentBatchProgress((batchProgress) => {
        setDocumentImport((current) => current.status === 'importing' && current.format === format
          ? { ...current, batchProgress }
          : current)
      })
      const batchResult = await importer()
      if (batchResult.imported.length > 0) await refreshUploadedLibrary()
      setDocumentImport({
        status: batchResult.cancelled ? 'cancelled' : 'imported',
        format,
        batchResult,
      })
      return batchResult
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setDocumentImport({
        status: cancelled ? 'cancelled' : 'error',
        format,
        message: cancelled ? undefined : message,
      })
      return null
    } finally {
      unlisten?.()
      operationInProgressRef.current = false
    }
  }, [refreshUploadedLibrary])

  const importDocumentBatch = useCallback(
    () => importDocumentCollection('batch', importDocumentBatchSource),
    [importDocumentCollection],
  )

  const importDocumentFolder = useCallback(
    () => importDocumentCollection('folder', importDocumentFolderSource),
    [importDocumentCollection],
  )

  /** Cancellation is cooperative: mark the UI only after Rust confirms that a
   * batch is active, then let the current file finish safely. */
  const cancelDocumentBatch = useCallback(async (): Promise<void> => {
    try {
      if (!await cancelDocumentBatchSource()) return
      setDocumentImport((current) => current.status === 'importing' &&
        (current.format === 'batch' || current.format === 'folder')
        ? { ...current, cancelRequested: true }
        : current)
    } catch (err) {
      console.warn('Unable to cancel document batch import:', err)
    }
  }, [])

  const deleteDocument = useCallback(async (doc: DocumentInfo): Promise<boolean> => {
    if (doc.source !== 'upload' || operationInProgressRef.current) return false

    operationInProgressRef.current = true
    setDocumentImport({ status: 'deleting', title: doc.title })
    try {
      const result = await deleteUploadedDocument(doc.url)
      await refreshUploadedLibrary()
      setDocumentImport({
        status: 'deleted',
        title: doc.title,
        bytesFreed: result.bytesFreed,
      })
      return true
    } catch (err) {
      setDocumentImport({
        status: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
      return false
    } finally {
      operationInProgressRef.current = false
    }
  }, [refreshUploadedLibrary])

  /** Serialize folder/order writes with imports and deletion because all of them
   * refresh or mutate the same uploaded-library state. */
  const runOrganizationMutation = useCallback(async (action: () => Promise<void>) => {
    if (operationInProgressRef.current) return
    operationInProgressRef.current = true
    try {
      await action()
    } finally {
      operationInProgressRef.current = false
    }
  }, [])

  const createLibraryFolder = useCallback((parentId: string | null, name: string) => runOrganizationMutation(async () => {
    await createUploadedLibraryFolder(parentId, name)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }), [runOrganizationMutation])

  const renameLibraryFolder = useCallback(async (folderId: string, name: string) => {
    await runOrganizationMutation(async () => {
      await renameUploadedLibraryFolder(folderId, name)
      setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
    })
  }, [runOrganizationMutation])

  const deleteLibraryFolder = useCallback(async (folderId: string) => {
    await runOrganizationMutation(async () => {
      await deleteUploadedLibraryFolder(folderId)
      setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
    })
  }, [runOrganizationMutation])

  const moveLibraryDocuments = useCallback(async (documentIds: string[], folderId: string | null) => {
    await runOrganizationMutation(async () => {
      setUploadedLibraryOrganization(await moveUploadedDocuments(documentIds, folderId))
    })
  }, [runOrganizationMutation])

  return {
    createLibraryFolder,
    cancelDocumentBatch,
    deleteDocument,
    deleteLibraryFolder,
    documentImport,
    importEpubDocument,
    importHtmlDocument,
    importDocumentBatch,
    importDocumentFolder,
    moveLibraryDocuments,
    renameLibraryFolder,
    uploadedDocuments,
    uploadedLibraryOrganization,
  }
}
