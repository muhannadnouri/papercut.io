import {
  useState,
  useEffect,
  useCallback,
  useMemo,
  useRef
} from 'react'
import './App.css'
import papercutIcon from './assets/papercut-icon.png'
import { usePagefind } from './hooks/usePagefind'
import { useSearch } from './hooks/useSearch'
import { SearchBar } from './components/SearchBar/SearchBar'
import { SearchResults } from './components/SearchResults/SearchResults'
import { DocumentsPanel } from './components/DocumentsPanel/DocumentsPanel'
import { DocumentViewer } from './components/DocumentViewer/DocumentViewer'
import { TabNav, type AppTab } from './components/TabNav/TabNav'
import { SearchScope } from './components/SearchScope/SearchScope'
import { ThemeToggle } from './components/ThemeToggle/ThemeToggle'
import { useAppConfirmation } from './components/AppDialog/useAppConfirmation'
import { useDocumentFilters } from './hooks/useDocumentFilters'
import { useTheme } from './hooks/useTheme'
import type { DocumentInfo } from './types/search'
import { clearPhraseFetchCache } from './utils/phraseSearch'
import { isDebugEnabled, setDebugEnabled } from './utils/debugFlags'
import { AudioControls } from './tts/components/AudioControls'
import { TtsDiagnosticsPanel } from './tts/components/TtsDiagnosticsPanel'
import { AudiobooksPanel } from './tts/components/AudiobooksPanel'
import { getImportedAudiobookSource } from './tts/api/nativeTts'
import { formatStorageSize } from './utils/formatUtils'
import { getUserUploads, isUserUploadUrl, type UserUploadDocument } from './tts/storage/UserUploads'
import { useAudiobookManager } from './tts/hooks/useAudiobookManager'
import {
  deleteUploadedDocument,
  createUploadedLibraryFolder,
  getUploadedDocumentSource,
  getUploadedLibraryOrganization,
  importEpubDocument,
  importHtmlDocument,
  isUploadedDocumentUrl,
  listUploadedDocuments,
  moveUploadedDocuments,
  renameUploadedLibraryFolder,
  deleteUploadedLibraryFolder,
  type UploadedDocument,
  type UploadedLibraryOrganization,
} from './uploads/DocumentUploads'

type DocumentLoadState =
  | { status: 'idle' }
  | { status: 'loading'; url: string; message: string }
  | { status: 'error'; url: string; message: string }

type UploadedLibraryState = {
  documents: UploadedDocument[]
  organization: UploadedLibraryOrganization
}

type BrowseScrollSnapshot = {
  activeTab: AppTab
  windowX: number
  windowY: number
  panelScrollTop: number | null
}

function getDocumentBrowserPanelBody(tab: AppTab): HTMLElement | null {
  const label = tab === 'search' ? 'Search' : tab === 'library' ? 'Library' : 'Audiobooks'
  const panel = document.querySelector(`.tab-panel[aria-label="${label}"] .document-browser-panel .panel-body`)
  return panel instanceof HTMLElement ? panel : null
}

function App() {
  const theme = useTheme()
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null)
  const [docContent, setDocContent] = useState('')
  const [documentLoad, setDocumentLoad] = useState<DocumentLoadState>({ status: 'idle' })
  const openDocumentRequestRef = useRef(0)
  const documentOpeningRef = useRef(false)
  const browseScrollRef = useRef<BrowseScrollSnapshot | null>(null)
  const [activeTab, setActiveTab] = useState<AppTab>('library')
  const [userUploads, setUserUploads] = useState<UserUploadDocument[]>(() => getUserUploads())
  const [uploadedDocuments, setUploadedDocuments] = useState<UploadedDocument[]>([])
  const [uploadedLibraryOrganization, setUploadedLibraryOrganization] = useState<UploadedLibraryOrganization>({ folders: [], documentLocations: [] })
  const [documentImport, setDocumentImport] = useState<{ status: 'idle' | 'importing' | 'imported' | 'deleting' | 'deleted' | 'cancelled' | 'error'; message: string }>({ status: 'idle', message: '' })
  const [ttsDiagnosticsEnabled, setTtsDiagnosticsEnabled] = useState(() => isDebugEnabled())
  const { pagefindRef, pagefindReady, allDocuments, documentsLoading } = usePagefind()
  const { confirm: confirmDocumentAction, dialog: documentConfirmationDialog } = useAppConfirmation()

  const loadHtmlDocument = useCallback(async (url: string): Promise<string> => {
    if (isUploadedDocumentUrl(url)) return getUploadedDocumentSource(url)
    if (isUserUploadUrl(url)) return getImportedAudiobookSource(url)

    const response = await fetch(url)
    if (!response.ok) throw new Error('Failed to load document')
    return response.text()
  }, [])

  const {
    query,
    results,
    loading,
    submittedQuery,
    lastSearchInfo,
    handleSearch,
    submitSearch,
    removeResultsForUrl,
  } = useSearch(pagefindRef, { loadDocumentSource: loadHtmlDocument })

  const loadUploadedLibrary = useCallback(async (): Promise<UploadedLibraryState> => {
    const [documents, organization] = await Promise.all([
      listUploadedDocuments(),
      getUploadedLibraryOrganization(),
    ])
    return { documents, organization }
  }, [])

  const applyUploadedLibrary = useCallback((library: UploadedLibraryState) => {
    setUploadedDocuments(library.documents)
    setUploadedLibraryOrganization(library.organization)
  }, [])

  const refreshUploadedLibrary = useCallback(async () => {
    applyUploadedLibrary(await loadUploadedLibrary())
  }, [applyUploadedLibrary, loadUploadedLibrary])

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
  }, [applyUploadedLibrary, loadUploadedLibrary])


  const clearSelectedDocument = useCallback(() => {
    openDocumentRequestRef.current += 1
    documentOpeningRef.current = false
    setSelectedDoc(null)
    setDocContent('')
    setDocumentLoad({ status: 'idle' })
  }, [])

  const handleUserUploadsChanged = useCallback(() => {
    setUserUploads(getUserUploads())
  }, [])

  const handleTtsDiagnosticsChange = useCallback((enabled: boolean) => {
    setDebugEnabled(enabled)
    setTtsDiagnosticsEnabled(enabled)
  }, [])

  useEffect(() => {
    if (selectedDoc) return
    const snapshot = browseScrollRef.current
    if (!snapshot || snapshot.activeTab !== activeTab) return

    browseScrollRef.current = null
    const frame = requestAnimationFrame(() => {
      window.scrollTo(snapshot.windowX, snapshot.windowY)
      const panelBody = getDocumentBrowserPanelBody(snapshot.activeTab)
      if (panelBody && snapshot.panelScrollTop !== null) {
        panelBody.scrollTop = snapshot.panelScrollTop
      }
    })

    return () => cancelAnimationFrame(frame)
  }, [activeTab, selectedDoc])

  const audiobook = useAudiobookManager({
    allDocuments,
    docContent,
    loadHtmlDocument,
    selectedDoc,
    uploadedDocuments,
    userUploads,
    onClearDocument: clearSelectedDocument,
    onUserUploadsChanged: handleUserUploadsChanged,
  })
  const {
    audioControlsProps,
    audioSetupProps,
    audiobookImport,
    audioSavedOnly,
    closeDocumentAudio,
    audiobooksPanelProps,
    filterResults,
    hasFloatingAudioControls,
    importAudiobook: importAudiobookBundle,
    includeDocumentInList,
    openSavedAudiobook,
    prepareDocumentOpen,
    setAudioSavedOnly,
    ttsHighlight,
  } = audiobook

  const libraryDocuments = useMemo<DocumentInfo[]>(() => [
    ...allDocuments.map((doc) => ({ ...doc, format: 'html', source: 'bundled' as const })),
    ...uploadedDocuments.map((upload) => ({ title: upload.title, url: upload.url, format: upload.format, source: 'upload' as const })),
    ...userUploads.map((upload) => ({ title: upload.title, url: upload.url, format: 'html', source: 'audiobook-upload' as const })),
  ], [allDocuments, uploadedDocuments, userUploads]) 

  const searchFilters = useDocumentFilters(libraryDocuments, { includeDocument: includeDocumentInList })
  const libraryFilters = useDocumentFilters(libraryDocuments, { includeDocument: includeDocumentInList })

  const {
    selectedFilters,
    documentFilter: searchDocumentFilter,
    collapsedAuthors: searchCollapsedAuthors,
    groupedDocs: searchGroupedDocs,
    docFilterLower: searchDocFilterLower,
    toggleFilter,
    clearFilters,
    removeFilter,
    toggleAuthor: toggleSearchAuthor,
    toggleAllInGroup,
    setDocumentFilter: setSearchDocumentFilter,
  } = searchFilters

  const {
    showDocuments,
    documentFilter: libraryDocumentFilter,
    collapsedAuthors: libraryCollapsedAuthors,
    groupedDocs: libraryGroupedDocs,
    docFilterLower: libraryDocFilterLower,
    toggleAuthor: toggleLibraryAuthor,
    setShowDocuments,
    setDocumentFilter: setLibraryDocumentFilter,
  } = libraryFilters 

  const audioFilteredResults = filterResults(results)
  const documentOpening = documentLoad.status === 'loading'

  const handleViewDocument = useCallback(async (url: string) => {
    if (documentOpeningRef.current) return
    documentOpeningRef.current = true
    const requestId = openDocumentRequestRef.current + 1
    openDocumentRequestRef.current = requestId
    const panelBody = getDocumentBrowserPanelBody(activeTab)
    browseScrollRef.current = {
      activeTab,
      windowX: window.scrollX,
      windowY: window.scrollY,
      panelScrollTop: panelBody?.scrollTop ?? null,
    }
    prepareDocumentOpen()
    setSelectedDoc(url)
    setDocContent('')
    setDocumentLoad({ status: 'loading', url, message: 'Opening Document...' })
    window.scrollTo({ top: 0 })

    try {
      const html = await loadHtmlDocument(url)
      if (openDocumentRequestRef.current !== requestId) return
      documentOpeningRef.current = false
      setDocContent(html)
      setDocumentLoad({ status: 'idle' })
    } catch (err) {
      if (openDocumentRequestRef.current !== requestId) return
      const message = err instanceof Error ? err.message : String(err)
      documentOpeningRef.current = false
      setDocContent('')
      setDocumentLoad({ status: 'error', url, message })
      console.error('Failed to load document:', err)
    }
  }, [activeTab, loadHtmlDocument, prepareDocumentOpen])

  const handleCloseDocument = useCallback(() => {
    closeDocumentAudio()
    clearSelectedDocument()
  }, [clearSelectedDocument, closeDocumentAudio])

  const handleTabChange = useCallback((tab: AppTab) => {
    setActiveTab(tab)
  }, [])

  const handleManageAudiobookSave = useCallback(() => {
    browseScrollRef.current = null
    clearSelectedDocument()
    setActiveTab('audiobooks')
    window.scrollTo({ top: 0 })
  }, [clearSelectedDocument])

  const selectedDocument = useMemo(
    () => (selectedDoc ? libraryDocuments.find((doc) => doc.url === selectedDoc) : undefined),
    [selectedDoc, libraryDocuments],
  )
  const selectedFormat = selectedDocument?.format

  const runDocumentImport = useCallback(async (
    importingMessage: string,
    importer: () => Promise<UploadedDocument>,
  ) => {
    setDocumentImport({ status: 'importing', message: importingMessage })
    try {
      const result = await importer()
      await refreshUploadedLibrary()
      setShowDocuments(true)
      setDocumentImport({ status: 'imported', message: 'Imported ' + result.title })
      await handleViewDocument(result.url)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setDocumentImport({
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled ? 'Import cancelled.' : message,
      })
    }
  }, [handleViewDocument, refreshUploadedLibrary, setShowDocuments])

  const handleImportHtmlDocument = useCallback(
    () => runDocumentImport('⏳ Importing HTML Document...', importHtmlDocument),
    [runDocumentImport],
  )

  const handleImportEpubDocument = useCallback(
    () => runDocumentImport('⏳ Importing EPUB Book...', importEpubDocument),
    [runDocumentImport],
  )

  const handleImportAudiobook = useCallback(async () => {
    await importAudiobookBundle(handleViewDocument)
  }, [handleViewDocument, importAudiobookBundle])

  const handleDeleteUploadedDocument = useCallback(async (doc: DocumentInfo) => {
    if (doc.source !== 'upload') return
    const confirmed = await confirmDocumentAction({
      title: 'Delete uploaded document?',
      description: 'This removes the document from this device, local search results, and any folder organization in Papercut.',
      details: [{ label: 'Title', value: doc.title }],
      confirmLabel: 'Delete Document',
      tone: 'danger',
    })
    if (!confirmed) return

    setDocumentImport({ status: 'deleting', message: 'Deleting ' + doc.title })
    try {
      const result = await deleteUploadedDocument(doc.url)
      await refreshUploadedLibrary()
      removeResultsForUrl(doc.url)
      clearPhraseFetchCache(doc.url)
      removeFilter(doc.title)
      if (selectedDoc === doc.url) {
        handleCloseDocument()
      }

      const storage = formatStorageSize(result.bytesFreed)
      setDocumentImport({
        status: 'deleted',
        message: storage ? 'Deleted ' + doc.title + ' and freed ' + storage + '.' : 'Deleted ' + doc.title + '.',
      })
    } catch (err) {
      setDocumentImport({
        status: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }, [confirmDocumentAction, handleCloseDocument, refreshUploadedLibrary, removeFilter, removeResultsForUrl, selectedDoc])

  const handleCreateLibraryFolder = useCallback(async (parentId: string | null, name: string) => {
    await createUploadedLibraryFolder(parentId, name)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const handleRenameLibraryFolder = useCallback(async (folderId: string, name: string) => {
    await renameUploadedLibraryFolder(folderId, name)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const handleDeleteLibraryFolder = useCallback(async (folderId: string) => {
    await deleteUploadedLibraryFolder(folderId)
    setUploadedLibraryOrganization(await getUploadedLibraryOrganization())
  }, [])

  const handleMoveLibraryDocuments = useCallback(async (documentIds: string[], folderId: string | null) => {
    setUploadedLibraryOrganization(await moveUploadedDocuments(documentIds, folderId))
  }, [])

  if (selectedDoc) {
    return (
      <>
        <DocumentViewer
          url={selectedDoc}
          format={selectedFormat}
          content={docContent}
          className={hasFloatingAudioControls ? 'app-audio-floating' : ''}
          appControls={<ThemeToggle choice={theme.choice} onChange={theme.setChoice} />}
          headerControls={<AudioControls {...audioControlsProps} onManageSave={handleManageAudiobookSave} />}
          beforeDocument={<TtsDiagnosticsPanel enabled={ttsDiagnosticsEnabled} />}
          ttsHighlight={ttsHighlight}
          loading={documentLoad.status === 'loading' && documentLoad.url === selectedDoc}
          loadError={documentLoad.status === 'error' && documentLoad.url === selectedDoc ? documentLoad.message : undefined}
          onClose={handleCloseDocument}
        />
        {documentConfirmationDialog}
        {audiobook.confirmationDialog}
      </>
    )
  }

  return (
    <div className="app">
      <header className="header">
        <div className="header-actions">
          <ThemeToggle choice={theme.choice} onChange={theme.setChoice} />
        </div>
        <h1 className="app-title">
          <img className="app-title-icon" src={papercutIcon} alt="" aria-hidden="true" />
          <span>Papercut</span>
        </h1>
        <p className="app-subtitle">Search, Read, & Listen Offline</p>
      </header>

      <TabNav
        active={activeTab}
        busyTabs={{ audiobooks: audiobooksPanelProps.isSaving }}
        onChange={handleTabChange}
      />

      {activeTab === 'search' && (
        <section className="tab-panel" role="tabpanel" aria-label="Search">
          <SearchBar
            query={query}
            disabled={!pagefindReady && uploadedDocuments.length === 0}
            onChange={handleSearch}
            onSubmit={submitSearch}
          />

          <SearchScope
            groupedDocs={searchGroupedDocs}
            collapsedAuthors={searchCollapsedAuthors}
            docFilterLower={searchDocFilterLower}
            documentFilter={searchDocumentFilter}
            libraryOrganization={uploadedLibraryOrganization}
            selectedFilters={selectedFilters}
            onFilterChange={setSearchDocumentFilter}
            onToggleFilter={toggleFilter}
            onToggleAllInGroup={toggleAllInGroup}
            onToggleAuthor={toggleSearchAuthor}
            onClearFilters={clearFilters}
          />

          <SearchResults
            results={audioFilteredResults}
            loading={loading}
            submittedQuery={submittedQuery}
            lastSearchInfo={lastSearchInfo}
            selectedFilters={selectedFilters}
            openingDisabled={documentOpening}
            openingDocumentUrl={documentLoad.status === 'loading' ? documentLoad.url : undefined}
            onViewResult={(result) => handleViewDocument(result.url)}
          />
        </section>
      )}

      {activeTab === 'library' && (
        <section className="tab-panel" role="tabpanel" aria-label="Library">
          <DocumentsPanel
            documentsLoading={documentsLoading}
            showDocuments={showDocuments}
            allDocuments={libraryDocuments}
            audioSavedOnly={audioSavedOnly}
            documentFilter={libraryDocumentFilter}
            groupedDocs={libraryGroupedDocs}
            docFilterLower={libraryDocFilterLower}
            importOptions={[
              {
                id: 'html',
                label: 'HTML',
                detail: 'Import a local .html or .htm document',
                statusLabel: documentImport.status === 'importing' && documentImport.message.includes('HTML') ? 'Importing HTML' : undefined,
                disabled: documentImport.status === 'importing',
                onSelect: handleImportHtmlDocument,
              },
              {
                id: 'epub',
                label: 'EPUB',
                detail: 'Import a local .epub book',
                statusLabel: documentImport.status === 'importing' && documentImport.message.includes('EPUB') ? 'Importing EPUB' : undefined,
                disabled: documentImport.status === 'importing',
                onSelect: handleImportEpubDocument,
              },
              // { id: 'pdf', label: 'PDF', detail: 'Import PDFs when text extraction support lands', future: true },
            ]}
            importStatuses={[documentImport]}
            libraryOrganization={uploadedLibraryOrganization}
            documentOpening={documentOpening}
            openingDocumentUrl={documentLoad.status === 'loading' ? documentLoad.url : undefined}
            collapsedAuthors={libraryCollapsedAuthors}
            onToggleShow={() => setShowDocuments((v) => !v)}
            onFilterChange={setLibraryDocumentFilter}
            onAudioSavedOnlyChange={setAudioSavedOnly}
            onCreateLibraryFolder={handleCreateLibraryFolder}
            onDeleteDocument={handleDeleteUploadedDocument}
            onDeleteLibraryFolder={handleDeleteLibraryFolder}
            onMoveLibraryDocuments={handleMoveLibraryDocuments}
            onRenameLibraryFolder={handleRenameLibraryFolder}
            onToggleAuthor={toggleLibraryAuthor}
            onViewDocument={handleViewDocument}
          />
        </section>
      )}

      {activeTab === 'audiobooks' && (
        <section className="tab-panel" role="tabpanel" aria-label="Audiobooks">
          <AudiobooksPanel
            {...audiobooksPanelProps}
            audioSetup={{
              ...audioSetupProps,
              debugEnabled: ttsDiagnosticsEnabled,
              onDiagnosticsChange: handleTtsDiagnosticsChange,
            }}
            importState={audiobookImport}
            documentOpening={documentOpening}
            onImportAudiobook={handleImportAudiobook}
            onOpenSaved={(record) => {
              void openSavedAudiobook(record, handleViewDocument)
            }}
          />

          <TtsDiagnosticsPanel enabled={ttsDiagnosticsEnabled} />
        </section>
      )}
      {documentConfirmationDialog}
      {audiobook.confirmationDialog}
    </div>
  )
}

export default App
