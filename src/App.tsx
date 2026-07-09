import {
  useState,
  useEffect,
  useCallback,
  useMemo
} from 'react'
import './App.css'
import { usePagefind } from './hooks/usePagefind'
import { useSearch } from './hooks/useSearch'
import { AppHeader } from './components/AppHeader/AppHeader'
import { SearchTab } from './components/SearchTab/SearchTab'
import { LibraryTab } from './components/LibraryTab/LibraryTab'
import { AudiobooksTab } from './components/AudiobooksTab/AudiobooksTab'
import { DocumentViewer } from './components/DocumentViewer/DocumentViewer'
import { TabNav, type AppTab } from './components/TabNav/TabNav'
import { ThemeToggle } from './components/ThemeToggle/ThemeToggle'
import { useAppConfirmation } from './components/AppDialog/useAppConfirmation'
import { useDocumentFilters } from './hooks/useDocumentFilters'
import { useDocumentViewerState } from './hooks/useDocumentViewerState'
import { useTheme } from './hooks/useTheme'
import { useUploadedLibrary } from './hooks/useUploadedLibrary'
import type { DocumentInfo, SearchOpenTarget } from './types/search'
import { clearPhraseFetchCache } from './utils/phraseSearch'
import { isDebugEnabled, setDebugEnabled } from './utils/debugFlags'
import { AudioControls } from './tts/components/AudioControls'
import { TtsDiagnosticsPanel } from './tts/components/TtsDiagnosticsPanel'
import { getImportedAudiobookSource } from './tts/api/nativeTts'
import { getUserUploads, isUserUploadUrl, type UserUploadDocument } from './tts/storage/UserUploads'
import { useAudiobookManager } from './tts/hooks/useAudiobookManager'
import {
  getUploadedDocumentSource,
  isUploadedDocumentUrl,
} from './uploads/DocumentUploads'

function App() {
  const theme = useTheme()
  const [activeTab, setActiveTab] = useState<AppTab>('library')
  const [userUploads, setUserUploads] = useState<UserUploadDocument[]>(() => getUserUploads())
  const [ttsDiagnosticsEnabled, setTtsDiagnosticsEnabled] = useState(() => isDebugEnabled())
  const { pagefindRef, pagefindReady, allDocuments, documentsLoading } = usePagefind()
  const { confirm: confirmDocumentAction, dialog: documentConfirmationDialog } = useAppConfirmation()
  const {
    createLibraryFolder,
    deleteDocument: deleteUploadedLibraryDocument,
    deleteLibraryFolder,
    documentImport,
    importEpubDocument,
    importHtmlDocument,
    moveLibraryDocuments,
    renameLibraryFolder,
    uploadedDocuments,
    uploadedLibraryOrganization,
  } = useUploadedLibrary()

  const loadHtmlDocument = useCallback(async (url: string): Promise<string> => {
    if (isUploadedDocumentUrl(url)) return getUploadedDocumentSource(url)
    if (isUserUploadUrl(url)) return getImportedAudiobookSource(url)

    const response = await fetch(url)
    if (!response.ok) throw new Error('Failed to load document')
    return response.text()
  }, [])

  const handleUserUploadsChanged = useCallback(() => {
    setUserUploads(getUserUploads())
  }, [])

  const handleTtsDiagnosticsChange = useCallback((enabled: boolean) => {
    setDebugEnabled(enabled)
    setTtsDiagnosticsEnabled(enabled)
  }, [])

  const {
    clearSelectedDocument,
    docContent,
    documentLoad,
    documentOpening,
    openDocument,
    restoreBookmark,
    searchOpenTarget,
    selectedDoc,
  } = useDocumentViewerState({
    activeTab,
    documentsLoading,
    loadHtmlDocument,
  })

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
    filterTitleByUrl: searchFilterTitleByUrl,
    toggleFilter,
    clearFilters,
    removeFilter,
    toggleAuthor: toggleSearchAuthor,
    toggleAllInGroup,
    setDocumentFilter: setSearchDocumentFilter,
  } = searchFilters

  const {
    query,
    results,
    loading,
    submittedQuery,
    lastSearchInfo,
    handleSearch,
    rerunSearch,
    submitSearch,
    removeResultsForUrl,
  } = useSearch(pagefindRef, { loadDocumentSource: loadHtmlDocument, scopeUrls: selectedFilters })

  useEffect(() => {
    rerunSearch()
  }, [rerunSearch])

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

  const handleViewDocument = useCallback((url: string, target?: SearchOpenTarget, options?: { restoreBookmark?: boolean }) => {
    return openDocument(url, target, options, prepareDocumentOpen)
  }, [openDocument, prepareDocumentOpen])

  const handleViewLibraryDocument = useCallback((url: string) => {
    return handleViewDocument(url, undefined, { restoreBookmark: true })
  }, [handleViewDocument])

  const handleCloseDocument = useCallback(() => {
    closeDocumentAudio()
    clearSelectedDocument()
  }, [clearSelectedDocument, closeDocumentAudio])

  const handleTabChange = useCallback((tab: AppTab) => {
    setActiveTab(tab)
  }, [])

  const handleManageAudiobookSave = useCallback(() => {
    clearSelectedDocument({ restoreBrowseScroll: false })
    setActiveTab('audiobooks')
    window.scrollTo({ top: 0 })
  }, [clearSelectedDocument])

  const selectedDocument = useMemo(
    () => (selectedDoc ? libraryDocuments.find((doc) => doc.url === selectedDoc) : undefined),
    [selectedDoc, libraryDocuments],
  )
  const selectedFormat = selectedDocument?.format

  const handleImportHtmlDocument = useCallback(async () => {
    const result = await importHtmlDocument()
    if (!result) return
    setShowDocuments(true)
    await handleViewDocument(result.url)
  }, [handleViewDocument, importHtmlDocument, setShowDocuments])

  const handleImportEpubDocument = useCallback(async () => {
    const result = await importEpubDocument()
    if (!result) return
    setShowDocuments(true)
    await handleViewDocument(result.url)
  }, [handleViewDocument, importEpubDocument, setShowDocuments])

  const handleImportAudiobook = useCallback(async () => {
    await importAudiobookBundle(handleViewDocument)
  }, [handleViewDocument, importAudiobookBundle])

  const handleToggleLibraryDocuments = useCallback(() => {
    setShowDocuments((value) => !value)
  }, [setShowDocuments])

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

    const deleted = await deleteUploadedLibraryDocument(doc)
    if (!deleted) return
    removeResultsForUrl(doc.url)
    clearPhraseFetchCache(doc.url)
    removeFilter(doc.url)
    if (selectedDoc === doc.url) {
      handleCloseDocument()
    }
  }, [confirmDocumentAction, deleteUploadedLibraryDocument, handleCloseDocument, removeFilter, removeResultsForUrl, selectedDoc])

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
          searchTarget={searchOpenTarget}
          restoreBookmark={restoreBookmark}
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
      <AppHeader actions={<ThemeToggle choice={theme.choice} onChange={theme.setChoice} />} />

      <TabNav
        active={activeTab}
        busyTabs={{ audiobooks: audiobooksPanelProps.isSaving }}
        onChange={handleTabChange}
      />

      {activeTab === 'search' && (
        <SearchTab
          query={query}
          disabled={!pagefindReady && uploadedDocuments.length === 0}
          onChangeQuery={handleSearch}
          onSubmitSearch={submitSearch}
          groupedDocs={searchGroupedDocs}
          collapsedAuthors={searchCollapsedAuthors}
          docFilterLower={searchDocFilterLower}
          documentFilter={searchDocumentFilter}
          libraryOrganization={uploadedLibraryOrganization}
          selectedFilters={selectedFilters}
          filterTitleByUrl={searchFilterTitleByUrl}
          onFilterChange={setSearchDocumentFilter}
          onToggleFilter={toggleFilter}
          onToggleAllInGroup={toggleAllInGroup}
          onToggleAuthor={toggleSearchAuthor}
          onClearFilters={clearFilters}
          results={audioFilteredResults}
          loading={loading}
          submittedQuery={submittedQuery}
          lastSearchInfo={lastSearchInfo}
          openingDisabled={documentOpening}
          openingDocumentUrl={documentLoad.status === 'loading' ? documentLoad.url : undefined}
          onViewResult={(result, target) => handleViewDocument(result.url, target)}
        />
      )}

      {activeTab === 'library' && (
        <LibraryTab
          documentsLoading={documentsLoading}
          showDocuments={showDocuments}
          allDocuments={libraryDocuments}
          audioSavedOnly={audioSavedOnly}
          documentFilter={libraryDocumentFilter}
          groupedDocs={libraryGroupedDocs}
          docFilterLower={libraryDocFilterLower}
          documentImport={documentImport}
          libraryOrganization={uploadedLibraryOrganization}
          documentOpening={documentOpening}
          openingDocumentUrl={documentLoad.status === 'loading' ? documentLoad.url : undefined}
          collapsedAuthors={libraryCollapsedAuthors}
          onToggleShow={handleToggleLibraryDocuments}
          onFilterChange={setLibraryDocumentFilter}
          onAudioSavedOnlyChange={setAudioSavedOnly}
          onCreateLibraryFolder={createLibraryFolder}
          onDeleteDocument={handleDeleteUploadedDocument}
          onDeleteLibraryFolder={deleteLibraryFolder}
          onMoveLibraryDocuments={moveLibraryDocuments}
          onRenameLibraryFolder={renameLibraryFolder}
          onToggleAuthor={toggleLibraryAuthor}
          onImportHtmlDocument={handleImportHtmlDocument}
          onImportEpubDocument={handleImportEpubDocument}
          onViewDocument={handleViewLibraryDocument}
        />
      )}

      {activeTab === 'audiobooks' && (
        <AudiobooksTab
          audiobooksPanelProps={audiobooksPanelProps}
          audioSetupProps={audioSetupProps}
          audiobookImport={audiobookImport}
          documentOpening={documentOpening}
          ttsDiagnosticsEnabled={ttsDiagnosticsEnabled}
          onDiagnosticsChange={handleTtsDiagnosticsChange}
          onImportAudiobook={handleImportAudiobook}
          onOpenSaved={(record) => {
            void openSavedAudiobook(record, handleViewDocument)
          }}
        />
      )}
      {documentConfirmationDialog}
      {audiobook.confirmationDialog}
    </div>
  )
}

export default App
