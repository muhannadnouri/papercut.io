import { useTranslation } from 'react-i18next'
import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'

interface DocumentListProps {
  groupedDocs: AuthorGroup[]
  collapsedAuthors: Set<string>
  docFilterLower: string
  onToggleAuthor: (author: string) => void
  emptyMessage?: string

  /** Selection mode (search scope): renders checkboxes + per-group select-all. */
  selectable?: boolean
  selectedFilters?: Set<string>
  onToggleFilter?: (url: string) => void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void

  /** Browse actions: render a View and/or Delete button per row. */
  onViewDocument?: (url: string) => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onDeleteDocument?: (doc: DocumentInfo) => void | Promise<void>
  deleteDisabled?: boolean
  openingDocumentUrl?: string
  viewDisabled?: boolean
}

/**
 * Author-grouped document list. Drives both the Library browse view
 * (View/Delete rows) and the Search scope selector (checkbox rows).
 */
export function DocumentList({
  groupedDocs,
  collapsedAuthors,
  docFilterLower,
  onToggleAuthor,
  emptyMessage,
  selectable = false,
  selectedFilters,
  onToggleFilter,
  onToggleAllInGroup,
  onViewDocument,
  onViewDocumentInfo,
  onDeleteDocument,
  deleteDisabled = false,
  openingDocumentUrl,
  viewDisabled = false,
}: DocumentListProps) {
  const { t, i18n } = useTranslation()
  if (groupedDocs.length === 0) {
    return (
      <div className="documents-scroll">
        <p className="no-results">{emptyMessage ?? t('library.documents.emptyFilter')}</p>
      </div>
    )
  }

  const isSelected = (url: string) => selectedFilters?.has(url) ?? false

  return (
    <div className="documents-scroll">
      {groupedDocs.map(({ author, docs }) => {
        const collapsed = docFilterLower.length === 0 && collapsedAuthors.has(author)
        const allSelected = selectable && docs.every((d) => isSelected(d.url))
        return (
          <div key={author} className="author-group">
            <div className="author-group-header">
              <button className="author-group-toggle" onClick={() => onToggleAuthor(author)}>
                <span className={'toggle-arrow ' + (collapsed ? '' : 'open')}>&#9662;</span>
                <bdi className="author-group-title">{author}</bdi>
                <span className="author-group-count">
                  ({docs.length.toLocaleString(i18n.resolvedLanguage ?? i18n.language)})
                </span>
              </button>
              {selectable && onToggleAllInGroup && (
                <button
                  className="author-group-action"
                  onClick={(e) => { e.stopPropagation(); onToggleAllInGroup(docs) }}
                >
                  {allSelected ? t('common.deselectAll') : t('common.selectAll')}
                </button>
              )}
            </div>

            {!collapsed && docs.map((doc) => (
              <DocumentRow
                key={doc.url}
                doc={doc}
                selectable={selectable}
                selected={isSelected(doc.url)}
                onToggleFilter={onToggleFilter}
                onViewDocument={onViewDocument}
                onViewDocumentInfo={onViewDocumentInfo}
                onDeleteDocument={onDeleteDocument}
                deleteDisabled={deleteDisabled}
                openingDocumentUrl={openingDocumentUrl}
                viewDisabled={viewDisabled}
              />
            ))}
          </div>
        )
      })}
    </div>
  )
}

interface DocumentRowProps {
  doc: DocumentInfo
  selectable: boolean
  selected: boolean
  onToggleFilter?: (url: string) => void
  onViewDocument?: (url: string) => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onDeleteDocument?: (doc: DocumentInfo) => void | Promise<void>
  deleteDisabled: boolean
  openingDocumentUrl?: string
  viewDisabled: boolean
}

function DocumentRow({
  doc,
  selectable,
  selected,
  onToggleFilter,
  onViewDocument,
  onViewDocumentInfo,
  onDeleteDocument,
  deleteDisabled,
  openingDocumentUrl,
  viewDisabled,
}: DocumentRowProps) {
  const { t } = useTranslation()
  const sourceIcon = doc.source === 'audiobook-upload' && (
    <span
      className="document-source-icon document-source-audiobook"
      aria-label={t('library.documents.audiobookImport')}
      title={t('library.documents.audiobookImport')}
    >
      🎧
    </span>
  )

  const opening = openingDocumentUrl === doc.url
  const disabled = viewDisabled || opening
  const view = onViewDocument && (
    <button
      className="document-row-action document-row-action-view"
      disabled={disabled}
      onClick={(e) => { e.preventDefault(); if (!disabled) onViewDocument(doc.url) }}
    >
      {opening ? t('common.opening') : t('common.view')}
    </button>
  )
  const remove = doc.source === 'upload' && onDeleteDocument && (
    <button
      className="document-row-action document-row-action-danger"
      disabled={deleteDisabled}
      onClick={(e) => { e.preventDefault(); void onDeleteDocument(doc) }}
    >
      {t('common.delete')}
    </button>
  )
  const info = doc.source === 'upload' && onViewDocumentInfo && (
    <button
      type="button"
      className="document-row-action document-row-action-secondary"
      onClick={(event) => { event.preventDefault(); onViewDocumentInfo(doc) }}
    >
      {t('library.documentInfo.button')}
    </button>
  )

  // Selection rows are labels so the whole row toggles the checkbox.
  if (selectable) {
    return (
      <label className={'document-item' + (selected ? ' document-item-selected' : '')}>
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onToggleFilter?.(doc.url)}
        />
        {sourceIcon}
        <bdi className="document-item-title">{doc.title}</bdi>
        {view}
        {remove}
      </label>
    )
  }

  return (
    <div className={'document-item document-item-browse' + (opening ? ' document-item-opening' : '')}>
      {sourceIcon}
      <bdi className="document-item-title">{doc.title}</bdi>
      {info}
      {view}
      {remove}
    </div>
  )
}
