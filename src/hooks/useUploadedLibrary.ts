import { useCallback, useEffect, useState } from 'react'
import type { DocumentInfo } from '../types/search'
import { formatStorageSize } from '../utils/formatUtils'
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

type DocumentImportStatus = {
  status: 'idle' | 'importing' | 'imported' | 'deleting' | 'deleted' | 'cancelled' | 'error'
  message: string
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
  const [documentImport, setDocumentImport] = useState<DocumentImportStatus>({ status: 'idle', message: '' })

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
    importingMessage: string,
    importer: () => Promise<UploadedDocument>,
  ): Promise<UploadedDocument | null> => {
    setDocumentImport({ status: 'importing', message: importingMessage })
    try {
      const result = await importer()
      await refreshUploadedLibrary()
      setDocumentImport({ status: 'imported', message: 'Imported ' + result.title })
      return result
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setDocumentImport({
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled ? 'Import cancelled.' : message,
      })
      return null
    }
  }, [refreshUploadedLibrary])

  const importHtmlDocument = useCallback(
    () => importDocument('⏳ Importing HTML Document...', importHtmlDocumentSource),
    [importDocument],
  )

  const importEpubDocument = useCallback(
    () => importDocument('⏳ Importing EPUB Book...', importEpubDocumentSource),
    [importDocument],
  )

  const deleteDocument = useCallback(async (doc: DocumentInfo): Promise<boolean> => {
    if (doc.source !== 'upload') return false

    setDocumentImport({ status: 'deleting', message: 'Deleting ' + doc.title })
    try {
      const result = await deleteUploadedDocument(doc.url)
      await refreshUploadedLibrary()
      const storage = formatStorageSize(result.bytesFreed)
      setDocumentImport({
        status: 'deleted',
        message: storage ? 'Deleted ' + doc.title + ' and freed ' + storage + '.' : 'Deleted ' + doc.title + '.',
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
