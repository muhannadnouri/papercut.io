import {
  useState,
  useEffect,
  useCallback,
  useMemo
} from 'react'
import { useTranslation } from 'react-i18next'
import './App.css'
import { usePagefind } from './hooks/usePagefind'
import { useSearch } from './hooks/useSearch'
import { AppHeader } from './components/AppHeader/AppHeader'
import { SearchTab } from './components/SearchTab/SearchTab'
import { LibraryTab } from './components/LibraryTab/LibraryTab'
import { AudiobooksTab } from './components/AudiobooksTab/AudiobooksTab'
import { DocumentViewer } from './components/DocumentViewer/DocumentViewer'
import { TabNav, type AppTab } from './components/TabNav/TabNav'
import { AppSettings } from './components/AppSettings/AppSettings'
import {
  TranslationPanel,
  type TranslationSeedDocument,
} from './translation/components/TranslationPanel'
import { useTranslationManager } from './translation/hooks/useTranslationManager'
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
  isUploadedHtmlDocumentUrl,
  isUploadedPdfDocumentUrl,
} from './uploads/DocumentUploads'

function isBundledDocumentUrl(url: string): boolean {
  try {
    const candidate = new URL(url, window.location.href)
    const current = new URL(window.location.href)
    return url.startsWith('/documents/') &&
      candidate.protocol === current.protocol &&
      candidate.host === current.host &&
      candidate.pathname.startsWith('/documents/')
  } catch {
    return false
  }
}

function App() {
  const { t } = useTranslation()
  const theme = useTheme()
  const [activeTab, setActiveTab] = useState<AppTab>('library')
  const [translationSeedDocument, setTranslationSeedDocument] =
    useState<TranslationSeedDocument | null>(null)
  const [userUploads, setUserUploads] = useState<UserUploadDocument[]>(() => getUserUploads())
  const [ttsDiagnosticsEnabled, setTtsDiagnosticsEnabled] = useState(() => isDebugEnabled())
  const { pagefindRef, pagefindReady, allDocuments, documentsLoading } = usePagefind()
  const { confirm: confirmDocumentAction, dialog: documentConfirmationDialog } = useAppConfirmation()
  const {
    cancelDocumentBatch,
    createLibraryFolder,
    deleteDocument: deleteUploadedLibraryDocument,
    deleteDocuments: deleteUploadedLibraryDocuments,
    deleteLibraryFolder,
    dismissDocumentImportStatus,
    documentImport,
    importDocumentBatch,
    importDocumentFolder,
    moveLibraryDocuments,
    refreshUploadedLibrary,
    renameLibraryFolder,
    uploadedDocuments,
    uploadedLibraryOrganization,
  } = useUploadedLibrary()

  const loadHtmlDocument = useCallback(async (url: string): Promise<string> => {
    if (isUploadedHtmlDocumentUrl(url)) return getUploadedDocumentSource(url)
    if (isUploadedPdfDocumentUrl(url)) return ''
    if (isUserUploadUrl(url)) return getImportedAudiobookSource(url)
    if (!isBundledDocumentUrl(url)) throw new Error('Unsupported document URL')

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
    onUploadedDocumentsChanged: refreshUploadedLibrary,
  })
  const {
    audioControlsProps,
    audioSetupProps,
    audiobookActionBusy,
    audiobookActionMessage,
    audiobookImport,
    audioSavedOnly,
    closeDocumentAudio,
    audiobooksPanelProps,
    hasFloatingAudioControls,
    importAudiobook: importAudiobookBundle,
    includeDocumentInList,
    openSavedAudiobook,
    prepareDocumentOpen,
    refreshSavedAudiobooks,
    savedAudiobookDocumentUrls,
    setAudioSavedOnly,
    ttsHighlight,
  } = audiobook

  const libraryDocuments = useMemo<DocumentInfo[]>(() => [
    ...allDocuments.map((doc) => ({ ...doc, format: 'html', source: 'bundled' as const })),
    ...uploadedDocuments.map((upload) => ({
      title: upload.title,
      url: upload.url,
      format: upload.format,
      source: 'upload' as const,
      coverMediaType: upload.coverMediaType,
    })),
    ...userUploads.map((upload) => ({ title: upload.title, url: upload.url, format: 'html', source: 'audiobook-upload' as const })),
  ], [allDocuments, uploadedDocuments, userUploads]) 

  const searchFilters = useDocumentFilters(libraryDocuments)
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

  const handleTranslationDocumentLibraryChanged = useCallback(async (changedDocumentUrl?: string) => {
    await refreshUploadedLibrary()
    if (!changedDocumentUrl) return
    removeResultsForUrl(changedDocumentUrl)
    clearPhraseFetchCache(changedDocumentUrl)
    removeFilter(changedDocumentUrl)
  }, [refreshUploadedLibrary, removeFilter, removeResultsForUrl])

  const translation = useTranslationManager({
    enabled: activeTab === 'translation',
    onDocumentLibraryChanged: handleTranslationDocumentLibraryChanged,
  })

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
  const selectedTitle = selectedDocument?.title
  const selectedFormat = selectedDocument?.format
  const canTranslateSelectedDocument =
    selectedDocument?.source === 'upload' && selectedFormat !== 'pdf'

  const handleRequestDocumentTranslation = useCallback(() => {
    if (!selectedDoc || !canTranslateSelectedDocument) return
    setTranslationSeedDocument({
      title: selectedTitle ?? selectedDoc,
      url: selectedDoc,
      format: selectedFormat,
    })
    closeDocumentAudio()
    clearSelectedDocument()
    setActiveTab('translation')
  }, [
    canTranslateSelectedDocument,
    clearSelectedDocument,
    closeDocumentAudio,
    selectedDoc,
    selectedFormat,
    selectedTitle,
  ])

  const handleImportDocumentBatch = useCallback(async () => {
    const result = await importDocumentBatch()
    if (!result?.imported.length) return
    setShowDocuments(true)
    if (result.selected === 1 &&
        result.imported.length === 1 &&
        result.imported[0].sourceKind === 'html') {
      await handleViewDocument(result.imported[0].url)
    }
  }, [handleViewDocument, importDocumentBatch, setShowDocuments])

  const handleImportDocumentFolder = useCallback(async () => {
    const result = await importDocumentFolder()
    if (result?.imported.length) setShowDocuments(true)
  }, [importDocumentFolder, setShowDocuments])

  const handleImportAudiobook = useCallback(async () => {
    await importAudiobookBundle(handleViewDocument)
  }, [handleViewDocument, importAudiobookBundle])

  const handleLibraryTransferImported = useCallback(async () => {
    await refreshUploadedLibrary()
    handleUserUploadsChanged()
    await refreshSavedAudiobooks()
  }, [handleUserUploadsChanged, refreshSavedAudiobooks, refreshUploadedLibrary])

  const handleToggleLibraryDocuments = useCallback(() => {
    setShowDocuments((value) => !value)
  }, [setShowDocuments])

  const handleDeleteUploadedDocument = useCallback(async (doc: DocumentInfo) => {
    if (doc.source !== 'upload') return
    const confirmed = await confirmDocumentAction({
      title: t('library.confirmDeleteDocument.title'),
      description: t('library.confirmDeleteDocument.description'),
      details: [{ label: t('library.confirmDeleteDocument.documentTitle'), value: <bdi>{doc.title}</bdi> }],
      confirmLabel: t('library.confirmDeleteDocument.confirm'),
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
  }, [confirmDocumentAction, deleteUploadedLibraryDocument, handleCloseDocument, removeFilter, removeResultsForUrl, selectedDoc, t])

  const handleDeleteUploadedDocuments = useCallback(async (docs: DocumentInfo[]) => {
    const result = await deleteUploadedLibraryDocuments(docs)
    if (!result) return null

    const deletedUrls = new Set(result.deleted.map((document) => document.url))
    for (const url of deletedUrls) {
      removeResultsForUrl(url)
      clearPhraseFetchCache(url)
      removeFilter(url)
    }
    if (selectedDoc && deletedUrls.has(selectedDoc)) handleCloseDocument()
    return result
  }, [deleteUploadedLibraryDocuments, handleCloseDocument, removeFilter, removeResultsForUrl, selectedDoc])

  if (selectedDoc) {
    return (
      <>
        <div inert={audiobookActionBusy ? true : undefined}>
          <DocumentViewer
            url={selectedDoc}
            format={selectedFormat}
            content={docContent}
            className={hasFloatingAudioControls ? 'app-audio-floating' : ''}
            appControls={(
              <AppSettings
                themeChoice={theme.choice}
                onThemeChange={theme.setChoice}
                developerMode={ttsDiagnosticsEnabled}
                onDeveloperModeChange={handleTtsDiagnosticsChange}
                libraryDocumentCount={uploadedDocuments.length}
                onLibraryImported={handleLibraryTransferImported}
              />
            )}
            headerControls={(
              <AudioControls
                {...audioControlsProps}
                canTranslateDocument={canTranslateSelectedDocument}
                onManageSave={handleManageAudiobookSave}
                onRequestTranslation={handleRequestDocumentTranslation}
              />
            )}
            beforeDocument={<TtsDiagnosticsPanel enabled={ttsDiagnosticsEnabled} />}
            ttsHighlight={ttsHighlight}
            searchTarget={searchOpenTarget}
            restoreBookmark={restoreBookmark}
            loading={documentLoad.status === 'loading' && documentLoad.url === selectedDoc}
            loadError={documentLoad.status === 'error' && documentLoad.url === selectedDoc ? documentLoad.message : undefined}
            onClose={handleCloseDocument}
          />
        </div>
        {audiobookActionBusy && <AppBusyOverlay message={audiobookActionMessage} />}
        {documentConfirmationDialog}
        {audiobook.confirmationDialog}
      </>
    )
  }

  return (
    <div className="app">
      <div
        className={audiobookActionBusy ? 'app-header-shell app-header-shell-busy' : 'app-header-shell'}
        inert={audiobookActionBusy ? true : undefined}
      >
        <AppHeader actions={(
          <AppSettings
            themeChoice={theme.choice}
            onThemeChange={theme.setChoice}
            developerMode={ttsDiagnosticsEnabled}
            onDeveloperModeChange={handleTtsDiagnosticsChange}
            libraryDocumentCount={uploadedDocuments.length}
            onLibraryImported={handleLibraryTransferImported}
          />
        )} />
      </div>

      <div inert={audiobookActionBusy ? true : undefined}>
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
            results={results}
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
            developerMode={ttsDiagnosticsEnabled}
            savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
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
            onDeleteDocuments={handleDeleteUploadedDocuments}
            onDeleteLibraryFolder={deleteLibraryFolder}
            onDismissDocumentImportStatus={dismissDocumentImportStatus}
            onMoveLibraryDocuments={moveLibraryDocuments}
            onRenameLibraryFolder={renameLibraryFolder}
            onToggleAuthor={toggleLibraryAuthor}
            onImportDocumentBatch={handleImportDocumentBatch}
            onImportDocumentFolder={handleImportDocumentFolder}
            onCancelDocumentBatch={cancelDocumentBatch}
            onViewDocument={handleViewLibraryDocument}
          />
        )}

        {activeTab === 'translation' && (
          <section className="tab-panel" role="tabpanel" aria-label={t('navigation.translation')}>
            <TranslationPanel
              {...translation}
              selectedDocument={translationSeedDocument}
              onOpenTranslatedDocument={handleViewDocument}
            />
          </section>
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
      </div>
      {audiobookActionBusy && <AppBusyOverlay message={audiobookActionMessage} />}
      {documentConfirmationDialog}
      {audiobook.confirmationDialog}
    </div>
  )
}

function AppBusyOverlay({ message }: { message: string }) {
  return (
    <div
      className="app-busy-overlay"
      role="status"
      aria-live="polite"
      aria-label={message}
    >
      <div className="app-busy-card">
        <span className="spinner" aria-hidden="true" />
        <span dir="auto">{message}</span>
      </div>
    </div>
  )
}

export default App
