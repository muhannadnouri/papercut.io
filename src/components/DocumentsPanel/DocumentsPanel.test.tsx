import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DocumentsPanel, type DocumentImportOption } from './DocumentsPanel'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en', resolvedLanguage: 'en' },
  }),
}))

afterEach(() => vi.unstubAllGlobals())

function renderImportButton(
  importOptions: DocumentImportOption[],
  importStatuses: Parameters<typeof DocumentsPanel>[0]['importStatuses'] = [],
) {
  return renderToStaticMarkup(
    <DocumentsPanel
      allDocuments={[]}
      collapsedAuthors={new Set()}
      docFilterLower=""
      documentFilter=""
      documentsLoading={false}
      groupedDocs={[]}
      importOptions={importOptions}
      importStatuses={importStatuses}
      showDocuments
      onFilterChange={() => undefined}
      onToggleAuthor={() => undefined}
      onToggleShow={() => undefined}
      onViewDocument={() => undefined}
    />,
  )
}

describe('document import progress', () => {
  it('replaces the disabled import label with an accessible spinner', () => {
    const html = renderImportButton([{
      id: 'files',
      label: 'Files',
      statusLabel: 'Importing documents',
      disabled: true,
    }], [{ status: 'importing', message: 'Importing 1 of 2' }])

    expect(html).toContain('aria-label="Importing documents"')
    expect(html).toContain('document-import-btn-spinner')
    expect(html).toContain('document-import-btn-label" aria-hidden="true"')
  })

  it('shows the generic busy state for drop and Open With imports', () => {
    const html = renderImportButton([{
      id: 'files',
      label: 'Files',
      disabled: true,
    }], [{ status: 'importing', message: 'Importing 1 of 2' }])

    expect(html).toContain('aria-label="library.import.importingBatch"')
    expect(html).toContain('document-import-btn-busy')
    expect(html).toContain('document-import-btn-spinner')
  })
})

describe('document list filters', () => {
  it('prunes unrelated upload folders for Saved Audio and renders folder affordances', () => {
    vi.stubGlobal('window', {
      localStorage: {
        getItem: (key: string) => key === 'papercut.library-view.v1' ? 'list' : null,
        setItem: () => undefined,
      },
    })
    const document = {
      title: 'Saved document',
      url: '/uploads/a1.html',
      format: 'html',
      source: 'upload' as const,
    }
    const html = renderToStaticMarkup(
      <DocumentsPanel
        allDocuments={[document]}
        audioSavedOnly
        collapsedAuthors={new Set()}
        docFilterLower=""
        documentFilter=""
        documentsLoading={false}
        groupedDocs={[{ author: 'library.groups.userUploads', docs: [document] }]}
        libraryOrganization={{
          folders: [
            { id: 'saved', name: 'Saved folder', depth: 0, sortOrder: 0, createdAtMs: 0, updatedAtMs: 0 },
            { id: 'empty', name: 'Unrelated folder', depth: 0, sortOrder: 1, createdAtMs: 0, updatedAtMs: 0 },
          ],
          documentLocations: [{ documentId: 'a1', folderId: 'saved', sortOrder: 0 }],
        }}
        showDocuments
        onAudioSavedOnlyChange={() => undefined}
        onCreateLibraryFolder={() => undefined}
        onDeleteDocuments={async () => null}
        onDeleteLibraryFolder={async () => null}
        onFilterChange={() => undefined}
        onMoveLibraryDocuments={() => undefined}
        onRenameLibraryFolder={() => undefined}
        onToggleAuthor={() => undefined}
        onToggleShow={() => undefined}
        onViewDocument={() => undefined}
      />,
    )

    expect(html).toContain('Saved folder')
    expect(html).not.toContain('Unrelated folder')
    expect(html).toContain('uploaded-library-folder-icon')
    expect(html).toContain('uploaded-library-chevron')
  })
})
