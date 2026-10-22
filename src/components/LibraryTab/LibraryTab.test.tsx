import type { ComponentProps, ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { UploadedDocument } from '../../uploads/DocumentUploads'
import { LibraryTab } from './LibraryTab'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) => options?.count === undefined
      ? key : `${key}:${options.count}`,
  }),
  Trans: () => null,
}))

vi.mock('../../i18n', () => ({ currentAppLocale: () => 'en' }))

vi.mock('../DocumentsPanel/DocumentsPanel', () => ({
  DocumentsPanel: ({ importStatuses }: {
    importStatuses: Array<{ message: ReactNode }>
  }) => <div>{importStatuses.map((status, index) => <div key={index}>{status.message}</div>)}</div>,
}))

/** Render the actual batch outcome through its normal Library entry point. */
function imported(documents: UploadedDocument[]) {
  const props: ComponentProps<typeof LibraryTab> = {
    allDocuments: [], audioSavedOnly: false, bookmarkedDocumentUrls: new Set(),
    collapsedAuthors: new Set(), docFilterLower: '', documentFilter: '',
    documentImport: {
      status: 'imported', format: 'batch', batchResult: {
        selected: documents.length, processed: documents.length, imported: documents,
        alreadyInLibrary: [], failures: [], cancelled: false,
      },
    },
    documentScannerSupported: false, documentPhotoImportSupported: false,
    documentOpening: false, documentsLoading: false, groupedDocs: [],
    libraryOrganization: { folders: [], documentLocations: [] },
    savedAudiobookDocumentUrls: new Set(), showDocuments: false,
    onAudioSavedOnlyChange: () => undefined,
    onAcceptRecognizedDocument: () => undefined,
    onCreateLibraryFolder: () => undefined,
    onDeleteDocument: () => undefined,
    onDeleteDocuments: async () => null,
    onDeleteLibraryFolder: async () => null,
    onDismissDocumentImportStatus: () => undefined,
    onFilterChange: () => undefined,
    onCancelDocumentBatch: () => undefined,
    onImportDocumentBatch: () => undefined,
    onImportDocumentFolder: () => undefined,
    onImportDocumentPaths: () => undefined,
    onImportDocumentPhotos: () => undefined,
    onImportPastedText: async () => undefined,
    onScanDocument: () => undefined,
    onMoveLibraryDocuments: () => undefined,
    onRecognizeDocument: () => undefined,
    onRenameLibraryFolder: () => undefined,
    onToggleAuthor: () => undefined,
    onToggleShow: () => undefined,
    onUpdateDocumentTitle: async () => undefined,
    onViewAudiobooks: () => undefined,
    onViewDocument: () => undefined,
  }
  return renderToStaticMarkup(<LibraryTab {...props} />)
}

function document(id: string, warnings?: string[]): UploadedDocument {
  return {
    id, url: `/uploads/${id}.html`, title: `Document ${id}`, format: 'html',
    sourceKind: 'html', importedAtMs: 0, bytes: 12, sections: 1, textStatus: 'ready',
    ...(warnings && { sourceFormatting: { policy: 'source-emphasis-v1', warnings } }),
  }
}

describe('source-formatting import warnings', () => {
  it('distinguishes styling limitations from image issues and renders filenames as text', () => {
    const source = document('source', ['unread-stylesheet', 'unsupported-value'])
    source.originalFileName = '<script>alert(1)</script>.html'
    source.importImageWarnings = { missing: 1 }
    const html = imported([source])

    expect(html).toContain('library.status.sourceFormattingWarnings')
    expect(html).toContain('library.status.sourceFormattingWarning')
    expect(html).toContain('imageIssues.summary:1')
    expect(html).toContain('<details class="document-batch-failures" open="">')
    expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;.html')
    expect(html).not.toContain('<script>')
    expect(html).not.toContain('unread-stylesheet')
    expect(html).not.toContain('unsupported-value')
  })

  it('does not warn for complete reports or legacy imports without evidence', () => {
    const html = imported([document('complete', []), document('legacy')])
    expect(html).not.toContain('library.status.sourceFormattingWarning')
  })

  it('bounds the affected-file list and reports the omitted file count', () => {
    const documents = Array.from({ length: 11 }, (_, index) =>
      document(`affected-${index}`, ['node-work-limit']))
    const html = imported([...documents, document('complete', [])])
    expect(html.match(/<li>/g)).toHaveLength(10)
    expect(html).toContain('Document affected-9')
    expect(html).not.toContain('Document affected-10')
    expect(html).toContain('library.status.sourceFormattingMore:1')
    expect(html).not.toContain('Document complete')
  })
})
