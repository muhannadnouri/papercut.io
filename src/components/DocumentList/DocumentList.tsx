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
  onToggleFilter?: (title: string) => void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void

  /** Browse actions: render a View and/or Delete button per row. */
  onViewDocument?: (url: string) => void
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
  emptyMessage = 'No documents match the filter.',
  selectable = false,
  selectedFilters,
  onToggleFilter,
  onToggleAllInGroup,
  onViewDocument,
  onDeleteDocument,
  deleteDisabled = false,
  openingDocumentUrl,
  viewDisabled = false,
}: DocumentListProps) {
  if (groupedDocs.length === 0) {
    return (
      <div className="documents-scroll">
        <p className="no-results">{emptyMessage}</p>
      </div>
    )
  }

  const isSelected = (title: string) => selectedFilters?.has(title) ?? false

  return (
    <div className="documents-scroll">
      {groupedDocs.map(({ author, docs }) => {
        const collapsed = docFilterLower.length === 0 && collapsedAuthors.has(author)
        const allSelected = selectable && docs.every((d) => isSelected(d.title))
        return (
          <div key={author} className="author-group">
            <div className="author-group-header">
              <button className="author-group-toggle" onClick={() => onToggleAuthor(author)}>
                <span className={'toggle-arrow ' + (collapsed ? '' : 'open')}>&#9662;</span>
                <span className="author-group-title">{author}</span>
                <span className="author-group-count">({docs.length})</span>
              </button>
              {selectable && onToggleAllInGroup && (
                <button
                  className="author-group-action"
                  onClick={(e) => { e.stopPropagation(); onToggleAllInGroup(docs) }}
                >
                  {allSelected ? 'Deselect All' : 'Select All'}
                </button>
              )}
            </div>

            {!collapsed && docs.map((doc) => (
              <DocumentRow
                key={doc.url}
                doc={doc}
                selectable={selectable}
                selected={isSelected(doc.title)}
                onToggleFilter={onToggleFilter}
                onViewDocument={onViewDocument}
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
  onToggleFilter?: (title: string) => void
  onViewDocument?: (url: string) => void
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
  onDeleteDocument,
  deleteDisabled,
  openingDocumentUrl,
  viewDisabled,
}: DocumentRowProps) {
  const sourceIcon = doc.source === 'audiobook-upload' && (
    <span
      className="document-source-icon document-source-audiobook"
      aria-label="Audiobook import, not indexed for search"
      title="Audiobook import, not indexed for search"
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
      {opening ? 'Opening...' : 'View'}
    </button>
  )
  const remove = doc.source === 'upload' && onDeleteDocument && (
    <button
      className="document-row-action document-row-action-danger"
      disabled={deleteDisabled}
      onClick={(e) => { e.preventDefault(); void onDeleteDocument(doc) }}
    >
      Delete
    </button>
  )

  // Selection rows are labels so the whole row toggles the checkbox.
  if (selectable) {
    return (
      <label className={'document-item' + (selected ? ' document-item-selected' : '')}>
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onToggleFilter?.(doc.title)}
        />
        {sourceIcon}
        <span className="document-item-title">{doc.title}</span>
        {view}
        {remove}
      </label>
    )
  }

  return (
    <div className={'document-item document-item-browse' + (opening ? ' document-item-opening' : '')}>
      {sourceIcon}
      <span className="document-item-title">{doc.title}</span>
      {view}
      {remove}
    </div>
  )
}
