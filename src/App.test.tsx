// @vitest-environment jsdom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import i18n from './i18n'
import App from './App'

const scene = vi.hoisted(() => ({
  selectedDoc: null as string | null,
  dismiss: vi.fn(),
  rerun: vi.fn(),
}))
vi.mock('./hooks/usePagefind', () => ({
  usePagefind: () => ({
    allDocuments: [],
    documentsLoading: false,
  }),
}))
vi.mock('./hooks/useUploadedLibrary', () => ({
  useUploadedLibrary: () => ({
    uploadedDocuments: [],
    documentImport: { status: 'idle' },
  }),
}))
vi.mock('./hooks/useDocumentViewerState', () => ({
  useDocumentViewerState: () => ({
    selectedDoc: scene.selectedDoc,
    docContent: '',
    documentLoad: { status: 'idle' },
  }),
}))
vi.mock('./hooks/useTheme', () => ({ useTheme: () => ({ choice: 'light' }) }))
vi.mock('./hooks/useSearch', () => ({ useSearch: () => ({ rerunSearch: scene.rerun }) }))
vi.mock('./hooks/useOpenDocumentRequests', () => ({ useOpenDocumentRequests: () => {} }))
vi.mock('./search-semantic/useSemanticSearch', () => ({ useSemanticSearch: () => ({ enabled: false }) }))
vi.mock('./search-semantic/useConceptualSearch', () => ({ useConceptualSearch: () => ({}) }))
vi.mock('./document-scanner/useDocumentScanner', () => ({
  useDocumentScanner: () => ({
    supported: false,
    photoImportSupported: false,
  }),
}))
vi.mock('./tts/playAnywhere/usePlayAnywhereReader', () => ({ usePlayAnywhereReader: () => ({}) }))
vi.mock('./tts/hooks/useAudiobookManager', () => ({
  useAudiobookManager: () => ({
    audioControlsProps: { ttsState: { currentChunkIndex: null } },
    audiobooksPanelProps: { isSaving: false },
    audiobookNotice: {
      status: 'error',
      message: 'Invalid audiobook bundle',
    },
    dismissAudiobookNotice: scene.dismiss,
    savedAudiobookDocumentUrls: new Set(),
  }),
}))
vi.mock('./components/AppHeader/AppHeader', () => ({ AppHeader: () => null }))
vi.mock('./components/AppSettings/AppSettings', () => ({ AppSettings: () => null }))
vi.mock('./components/LibraryTab/LibraryTab', () => ({ LibraryTab: () => <div>Library surface</div> }))
vi.mock('./components/SearchTab/SearchTab', () => ({ SearchTab: () => <div>Search surface</div> }))
vi.mock('./components/AudiobooksTab/AudiobooksTab', () => ({ AudiobooksTab: () => <div>Audiobooks surface</div> }))
vi.mock('./components/DocumentViewer/DocumentViewer', () => ({ DocumentViewer: () => <div>Reader surface</div> }))
vi.mock('./tts/components/AudioControls', () => ({ AudioControls: () => null }))

afterEach(() => {
  vi.clearAllMocks()
  vi.unstubAllGlobals()
  localStorage.clear()
  document.body.replaceChildren()
})

it.each(['library', 'search', 'audiobooks', 'reader'])('shows one dismissible import failure on the %s surface', async surface => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  scene.selectedDoc = surface === 'reader' ? '/documents/book.html' : null
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(<App />))
    if (surface === 'search' || surface === 'audiobooks') {
      const label = i18n.t(`navigation.${surface}` as const)
      const tab = Array.from(host.querySelectorAll<HTMLButtonElement>('[role="tab"]'))
        .find(button => button.textContent?.includes(label))!
      await act(async () => tab.click())
    }
    expect(host.textContent).toContain(surface[0].toUpperCase() + surface.slice(1) + ' surface')
    expect(host.querySelectorAll('[role="alert"]')).toHaveLength(1)
    expect(host.querySelector('[role="alert"]')!.textContent).toContain('Invalid audiobook bundle')
    await act(async () => host.querySelector<HTMLButtonElement>('.audiobook-action-toast-dismiss')!.click())
    expect(scene.dismiss).toHaveBeenCalledOnce()
  } finally {
    await act(async () => root.unmount())
  }
})
