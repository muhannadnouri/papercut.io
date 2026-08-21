import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { DocumentsPanel, type DocumentImportOption } from './DocumentsPanel'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

function renderImportButton(importOptions: DocumentImportOption[]) {
  return renderToStaticMarkup(
    <DocumentsPanel
      allDocuments={[]}
      collapsedAuthors={new Set()}
      docFilterLower=""
      documentFilter=""
      documentsLoading={false}
      groupedDocs={[]}
      importOptions={importOptions}
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
    }])

    expect(html).toContain('aria-label="Importing documents"')
    expect(html).toContain('document-import-btn-spinner')
    expect(html).toContain('document-import-btn-label" aria-hidden="true"')
  })
})
