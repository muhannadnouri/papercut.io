import { useCallback, useEffect, useState } from 'react'
import type { DocumentInfo } from '../types/search'
import {
  createUploadedLibraryFolder,
  deleteUploadedDocument,
  deleteUploadedLibraryFolder,
  getUploadedLibraryOrganization,
  importEpubDocument as importEpubDocumentSource,
  importHtmlDocument as importHtmlDocumentSource,
  listUploadedDocuments,
  moveUploadedDocuments,
  renameUploadedLibraryFolder,
  type UploadedDocument,
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
  format?: 'html' | 'epub'
  title?: string
  bytesFreed?: number
  message?: string
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

  const deleteDocument = useCallback(async (doc: DocumentInfo): Promise<boolean> => {
    if (doc.source !== 'upload') return false

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
    }
  }, [refreshUploadedLibrary])

  const createLibraryFolder = useCallback(async (parentId: string | null, name: string) => {
    await createUploadedLibraryFolder(parentId, name)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const renameLibraryFolder = useCallback(async (folderId: string, name: string) => {
    await renameUploadedLibraryFolder(folderId, name)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const deleteLibraryFolder = useCallback(async (folderId: string) => {
    await deleteUploadedLibraryFolder(folderId)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const moveLibraryDocuments = useCallback(async (documentIds: string[], folderId: string | null) => {
    setUploadedLibraryOrganization(await moveUploadedDocuments(documentIds, folderId))
  }, [])

  return {
    createLibraryFolder,
    deleteDocument,
    deleteLibraryFolder,
    documentImport,
    importEpubDocument,
    importHtmlDocument,
    moveLibraryDocuments,
    renameLibraryFolder,
    uploadedDocuments,
    uploadedLibraryOrganization,
  }
}
