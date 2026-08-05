import { useCallback, useEffect, useRef, useState } from 'react'
import type { DocumentInfo } from '../types/search'
import {
  importDocumentPhotos as importDocumentPhotosSource,
  scanDocument as scanDocumentSource,
  type DocumentScanSetup,
} from '../document-scanner/documentScanner'
import { indexImportedPdfs } from '../pdf/pdfImport'
import {
  PDF_OCR_NO_TEXT,
  recognizeEnglishPdfDocument,
  type PdfRecognitionProgress,
} from '../pdf/ocr/recognizePdf'
import {
  cancelDocumentBatch as cancelDocumentBatchSource,
  createUploadedLibraryFolder,
  deleteUploadedDocument,
  deleteUploadedDocuments,
  deleteUploadedLibraryFolder,
  getUploadedLibraryOrganization,
  importDocumentBatch as importDocumentBatchSource,
  importDocumentFolder as importDocumentFolderSource,
  listUploadedDocuments,
  listenDocumentBatchProgress,
  listenDocumentDeleteProgress,
  moveUploadedDocuments,
  renameUploadedLibraryFolder,
  updateUploadedDocumentTitle,
  type UploadedDocument,
  type UploadedDocumentBatchProgress,
  type UploadedDocumentBatchResult,
  type UploadedDocumentDeleteBatchProgress,
  type UploadedDocumentDeleteBatchResult,
  type UploadedLibraryOrganization,
} from '../uploads/DocumentUploads'

type UploadedLibraryState = {
  documents: UploadedDocument[]
  organization: UploadedLibraryOrganization
}

const IMPORT_NOTICE_MS = 6000
export const LIBRARY_OPERATION_IN_PROGRESS = 'library-operation-in-progress'

// Keep operation state locale-neutral so the owning UI can translate it and
// isolate user titles without parsing preformatted English messages.
export type DocumentImportStatus = {
  status: 'idle' | 'importing' | 'imported' | 'recognizing' | 'recognized' | 'deleting' | 'deleted' | 'cancelled' | 'error'
  format?: 'batch' | 'folder' | 'scan' | 'photos' | 'pdf-ocr' | 'delete-batch'
  title?: string
  bytesFreed?: number
  message?: string
  batchProgress?: UploadedDocumentBatchProgress
  batchResult?: UploadedDocumentBatchResult
  deleteProgress?: UploadedDocumentDeleteBatchProgress
  deleteResult?: UploadedDocumentDeleteBatchResult
  recognitionProgress?: PdfRecognitionProgress
  cancelRequested?: boolean
}

interface DocumentCollectionImportOptions {
  recognitionLanguage?: DocumentScanSetup['recognitionLanguage']
  titleOverride?: string
}

/** Auto-dismiss only outcomes that have no file-level failure details to retain. */
export function shouldAutoDismissDocumentImport(status: DocumentImportStatus): boolean {
  if (status.status === 'recognized') return true
  if (status.status !== 'imported' && status.status !== 'cancelled') return false
  return (status.batchResult?.failures.length ?? 0) === 0
}

export function shouldRecognizeImportedScan(
  document: UploadedDocument,
  language?: DocumentScanSetup['recognitionLanguage'],
): boolean {
  return language === 'english' &&
    document.sourceKind === 'pdf' &&
    document.textStatus === 'recognition-required'
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
  const importAbortRef = useRef<AbortController | null>(null)

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

  useEffect(() => {
    if (!shouldAutoDismissDocumentImport(documentImport)) return
    const timer = window.setTimeout(() => {
      setDocumentImport((current) => current === documentImport ? { status: 'idle' } : current)
    }, IMPORT_NOTICE_MS)
    return () => window.clearTimeout(timer)
  }, [documentImport])

  const dismissDocumentImportStatus = useCallback(() => {
    setDocumentImport({ status: 'idle' })
  }, [])

  /** Subscribe before opening native selection UI so the shared import paths
   * retain even their first progress event and use one partial-result flow. */
  const importDocumentCollection = useCallback(async (
    format: 'batch' | 'folder' | 'scan' | 'photos',
    importer: () => Promise<UploadedDocumentBatchResult>,
    options: DocumentCollectionImportOptions = {},
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
      const abort = new AbortController()
      importAbortRef.current = abort
      let batchResult = await indexImportedPdfs(await importer(), {
        signal: abort.signal,
        titleOverride: options.titleOverride,
        onProgress: (batchProgress) => {
          setDocumentImport((current) => current.status === 'importing' && current.format === format
            ? { ...current, batchProgress }
            : current)
        },
      })
      const recognitionCandidate = batchResult.imported.find((document) => (
        shouldRecognizeImportedScan(document, options.recognitionLanguage)
      ))
      if (recognitionCandidate) {
        setDocumentImport({
          status: 'recognizing',
          format: 'pdf-ocr',
          title: recognitionCandidate.title,
        })
        try {
          const recognized = await recognizeEnglishPdfDocument(recognitionCandidate, {
            signal: abort.signal,
            onProgress: (recognitionProgress) => {
              setDocumentImport((current) => current.status === 'recognizing' && current.format === 'pdf-ocr'
                ? { ...current, recognitionProgress }
                : current)
            },
          })
          batchResult = {
            ...batchResult,
            imported: batchResult.imported.map((document) => (
              document.id === recognized.id ? recognized : document
            )),
          }
          await refreshUploadedLibrary()
          setDocumentImport({ status: 'recognized', format: 'pdf-ocr', title: recognized.title })
          return batchResult
        } catch (err) {
          await refreshUploadedLibrary()
          const cancelled = abort.signal.aborted || (err instanceof DOMException && err.name === 'AbortError')
          const message = err instanceof Error ? err.message : String(err)
          setDocumentImport({
            status: cancelled ? 'cancelled' : 'error',
            format: 'pdf-ocr',
            title: recognitionCandidate.title,
            message: message === PDF_OCR_NO_TEXT ? PDF_OCR_NO_TEXT : cancelled ? undefined : message,
          })
          return batchResult
        }
      }
      if (batchResult.imported.length > 0) await refreshUploadedLibrary()
      setDocumentImport({
        status: batchResult.failures.length > 0
          ? 'error'
          : batchResult.cancelled ? 'cancelled' : 'imported',
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
      importAbortRef.current = null
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

  const scanDocument = useCallback(
    (setup: DocumentScanSetup) => importDocumentCollection(
      'scan',
      () => scanDocumentSource(setup),
      { recognitionLanguage: setup.recognitionLanguage, titleOverride: setup.title },
    ),
    [importDocumentCollection],
  )

  const importDocumentPhotos = useCallback(
    (setup: DocumentScanSetup) => importDocumentCollection(
      'photos',
      () => importDocumentPhotosSource(setup),
      { recognitionLanguage: setup.recognitionLanguage, titleOverride: setup.title },
    ),
    [importDocumentCollection],
  )

  /** Run the opt-in English recognizer for one PDF with missing text, then
   * replace its page index through the normal finalizer. */
  const recognizeDocumentText = useCallback(async (documentUrl: string): Promise<boolean> => {
    const document = uploadedDocuments.find((candidate) => candidate.url === documentUrl)
    if (!document || operationInProgressRef.current) return false

    operationInProgressRef.current = true
    const abort = new AbortController()
    importAbortRef.current = abort
    setDocumentImport({ status: 'recognizing', format: 'pdf-ocr', title: document.title })
    try {
      const updated = await recognizeEnglishPdfDocument(document, {
        signal: abort.signal,
        onProgress: (recognitionProgress) => {
          setDocumentImport((current) => current.status === 'recognizing' && current.format === 'pdf-ocr'
            ? { ...current, recognitionProgress }
            : current)
        },
      })
      setUploadedDocuments((documents) => documents.map((candidate) => (
        candidate.id === updated.id ? updated : candidate
      )))
      setDocumentImport({ status: 'recognized', format: 'pdf-ocr', title: updated.title })
      return true
    } catch (err) {
      const cancelled = abort.signal.aborted || (err instanceof DOMException && err.name === 'AbortError')
      const message = err instanceof Error ? err.message : String(err)
      setDocumentImport({
        status: cancelled ? 'cancelled' : 'error',
        format: 'pdf-ocr',
        title: document.title,
        message: message === PDF_OCR_NO_TEXT ? PDF_OCR_NO_TEXT : cancelled ? undefined : message,
      })
      return false
    } finally {
      importAbortRef.current = null
      operationInProgressRef.current = false
    }
  }, [uploadedDocuments])

  /** Cancellation aborts frontend PDF work immediately and asks a native batch,
   * when present, to stop safely between files. */
  const cancelDocumentBatch = useCallback(async (): Promise<void> => {
    try {
      const abort = importAbortRef.current
      abort?.abort()
      const nativeCancelled = await cancelDocumentBatchSource()
      if (!abort && !nativeCancelled) return
      setDocumentImport((current) => (current.status === 'importing' &&
        (current.format === 'batch' || current.format === 'folder' || current.format === 'scan' ||
          current.format === 'photos')) ||
        (current.status === 'recognizing' && current.format === 'pdf-ocr')
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

  const updateDocumentTitle = useCallback(async (
    documentUrl: string,
    title: string,
  ): Promise<UploadedDocument> => {
    if (operationInProgressRef.current) {
      throw new Error(LIBRARY_OPERATION_IN_PROGRESS)
    }
    operationInProgressRef.current = true
    try {
      const updated = await updateUploadedDocumentTitle(documentUrl, title)
      setUploadedDocuments((documents) => documents.map((document) => (
        document.id === updated.id ? updated : document
      )))
      return updated
    } finally {
      operationInProgressRef.current = false
    }
  }, [])

  /** Run one bounded native delete batch and refresh shared library state once,
   * retaining partial failures for the selection UI to offer a retry. */
  const deleteDocuments = useCallback(async (
    documents: DocumentInfo[],
  ): Promise<UploadedDocumentDeleteBatchResult | null> => {
    const documentUrls = documents
      .filter((doc) => doc.source === 'upload')
      .map((doc) => doc.url)
    if (documentUrls.length === 0 || operationInProgressRef.current) return null

    operationInProgressRef.current = true
    setDocumentImport({ status: 'deleting', format: 'delete-batch' })
    let unlisten: (() => void) | undefined
    try {
      unlisten = await listenDocumentDeleteProgress((deleteProgress) => {
        setDocumentImport((current) => current.status === 'deleting' && current.format === 'delete-batch'
          ? { ...current, deleteProgress }
          : current)
      })
      const deleteResult = await deleteUploadedDocuments(documentUrls)
      if (deleteResult.deleted.length > 0) await refreshUploadedLibrary()
      setDocumentImport({
        status: deleteResult.failures.length > 0 ? 'error' : 'deleted',
        format: 'delete-batch',
        deleteResult,
      })
      return deleteResult
    } catch (err) {
      setDocumentImport({
        status: 'error',
        format: 'delete-batch',
        message: err instanceof Error ? err.message : String(err),
      })
      return null
    } finally {
      unlisten?.()
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
    deleteDocuments,
    deleteLibraryFolder,
    dismissDocumentImportStatus,
    documentImport,
    importDocumentBatch,
    importDocumentFolder,
    importDocumentPhotos,
    scanDocument,
    moveLibraryDocuments,
    refreshUploadedLibrary,
    recognizeDocumentText,
    renameLibraryFolder,
    uploadedDocuments,
    uploadedLibraryOrganization,
    updateDocumentTitle,
  }
}
