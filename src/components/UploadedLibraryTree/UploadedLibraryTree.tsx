import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Button, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import {
  type UploadedLibraryFolder,
  type UploadedLibraryOrganization,
  isUploadedDocumentUrl,
} from '../../uploads/DocumentUploads'
import { useAppConfirmation } from '../AppDialog/useAppConfirmation'
import { TextInputDialog } from '../TextInputDialog/TextInputDialog'
import './UploadedLibraryTree.css'

interface UploadedLibraryTreeProps {
  documents: DocumentInfo[]
  organization: UploadedLibraryOrganization
  mode?: 'library' | 'filter'
  documentOpening?: boolean
  openingDocumentUrl?: string
  selectedFilters?: Set<string>
  onCreateFolder?: (parentId: string | null, name: string) => Promise<void> | void
  onDeleteDocument?: (doc: DocumentInfo) => Promise<void> | void
  onDeleteFolder?: (folderId: string) => Promise<void> | void
  onMoveDocuments?: (documentIds: string[], folderId: string | null) => Promise<void> | void
  onRenameFolder?: (folderId: string, name: string) => Promise<void> | void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void
  onToggleFilter?: (url: string) => void
  onViewDocument?: (url: string) => void
}

type LibraryNode =
  | {
      key: string
      kind: 'folder'
      id: string
      title: string
      depth: number
      documentCount: number
      children: LibraryNode[]
    }
  | {
      key: string
      kind: 'document'
      id: string
      title: string
      url: string
      doc: DocumentInfo
      children: LibraryNode[]
    }

type FolderDialogState =
  | { kind: 'create'; parentId: string | null; parentName?: string }
  | { kind: 'rename'; folderId: string; initialName: string }

export function UploadedLibraryTree({
  documents,
  organization,
  mode = 'library',
  documentOpening = false,
  openingDocumentUrl,
  selectedFilters,
  onCreateFolder,
  onDeleteFolder,
  onDeleteDocument,
  onMoveDocuments,
  onRenameFolder,
  onToggleAllInGroup,
  onToggleFilter,
  onViewDocument,
}: UploadedLibraryTreeProps) {
  const { t, i18n } = useTranslation()
  const [editMode, setEditMode] = useState(false)
  const [selectedKeys, setSelectedKeys] = useState<Set<Key>>(new Set())
  const [expandedKeys, setExpandedKeys] = useState<Set<Key>>(new Set())
  const [rootCollapsed, setRootCollapsed] = useState(false)
  const [folderDialog, setFolderDialog] = useState<FolderDialogState | null>(null)
  const [folderDialogError, setFolderDialogError] = useState('')
  const [deleteInfoOpen, setDeleteInfoOpen] = useState(false)
  const [actionError, setActionError] = useState('')
  const [busy, setBusy] = useState(false)
  const { confirm: confirmLibraryAction, dialog: libraryConfirmationDialog } = useAppConfirmation()
  const filterMode = mode === 'filter'
  const organizing = mode === 'library' && editMode
  const locale = i18n.resolvedLanguage ?? i18n.language
  const { nodes, folders, nodeByKey, folderOptions } = useMemo(
    () => buildLibraryTree(documents, organization, { hideEmptyFolders: filterMode, locale }),
    [documents, filterMode, locale, organization],
  )
  const rootDocuments = useMemo(() => nodes.flatMap(collectDocuments), [nodes])
  const allRootSelected = rootDocuments.length > 0 && rootDocuments.every((doc) => selectedFilters?.has(doc.url))
  const selectedNodes = Array.from(selectedKeys)
    .map((key) => nodeByKey.get(String(key)))
    .filter((node): node is LibraryNode => Boolean(node))
  const selectedDocumentIds = selectedNodes
    .filter((node) => node.kind === 'document')
    .map((node) => node.id)
  const selectedFolders = selectedNodes.filter((node) => node.kind === 'folder')
  const hasMixedSelection = selectedDocumentIds.length > 0 && selectedFolders.length > 0
  const canMoveDocuments = organizing && selectedDocumentIds.length > 0 && selectedFolders.length === 0
  const selectedSingleFolder = selectedFolders.length === 1 && selectedDocumentIds.length === 0
  const selectedFolder = selectedSingleFolder ? selectedFolders[0] : undefined
  const selectedFolderHasContents = Boolean(selectedFolder && (
    selectedFolder.documentCount > 0 || selectedFolder.children.length > 0
  ))
  const canDeleteSelectedFolder = Boolean(selectedFolder && !selectedFolderHasContents && !busy)
  const deleteFolderBlocked = Boolean(selectedFolderHasContents && !busy)
  const deleteFolderHelp = !selectedSingleFolder
    ? t('library.tree.selectOneFolder')
    : selectedFolderHasContents
      ? t('library.tree.moveOrRemoveContents')
      : t('library.tree.deleteSelectedFolder')

  useEffect(() => {
    setDeleteInfoOpen(false)
    setActionError('')
  }, [selectedKeys, editMode])

  const runEditAction = async (action: () => Promise<void> | void) => {
    setBusy(true)
    try {
      await action()
      setSelectedKeys(new Set())
    } finally {
      setBusy(false)
    }
  }

  const toggleSelection = (key: string) => {
    setSelectedKeys((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  const toggleFolderExpanded = (key: string) => {
    setExpandedKeys((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  const handleAction = (key: Key) => {
    if (filterMode || editMode || documentOpening) return
    const node = nodeByKey.get(String(key))
    if (node?.kind === 'document') onViewDocument?.(node.url)
  }

  const openFolderDialog = (parentId: string | null, parentName?: string) => {
    setFolderDialogError('')
    setActionError('')
    setDeleteInfoOpen(false)
    setFolderDialog({ kind: 'create', parentId, parentName })
  }

  const submitFolderDialog = (name: string) => {
    if (!folderDialog) return
    const target = folderDialog
    void (async () => {
      try {
        if (target.kind === 'create') {
          if (!onCreateFolder) return
          await runEditAction(() => onCreateFolder(target.parentId, name))
        } else {
          if (!onRenameFolder) return
          if (name === target.initialName) {
            setFolderDialog(null)
            setFolderDialogError('')
            return
          }
          await runEditAction(() => onRenameFolder(target.folderId, name))
        }
        setFolderDialog(null)
        setFolderDialogError('')
      } catch (err) {
        setFolderDialogError(err instanceof Error ? err.message : String(err))
      }
    })()
  }

  const renameSelectedFolder = () => {
    if (!selectedSingleFolder) return
    const folder = selectedFolders[0]
    setFolderDialogError('')
    setActionError('')
    setDeleteInfoOpen(false)
    setFolderDialog({ kind: 'rename', folderId: folder.id, initialName: folder.title })
  }

  const deleteSelectedFolder = () => {
    if (deleteFolderBlocked) {
      setDeleteInfoOpen((value) => !value)
      return
    }
    if (!selectedSingleFolder) return
    if (!canDeleteSelectedFolder || !selectedFolder) return
    const folder = selectedFolder
    if (!onDeleteFolder) return
    setActionError('')
    void (async () => {
      const confirmed = await confirmLibraryAction({
        title: t('library.confirmDeleteFolder.title'),
        description: t('library.confirmDeleteFolder.description'),
        details: [{ label: t('library.confirmDeleteFolder.folder'), value: <bdi>{folder.title}</bdi> }],
        confirmLabel: t('library.tree.deleteFolder'),
        tone: 'danger',
      })
      if (!confirmed) return

      try {
        await runEditAction(() => onDeleteFolder(folder.id))
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err))
      }
    })()
  }

  const moveSelectedDocuments = (folderId: string | null) => {
    if (!canMoveDocuments) return
    if (!onMoveDocuments) return
    setActionError('')
    void (async () => {
      try {
        await runEditAction(() => onMoveDocuments(selectedDocumentIds, folderId))
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err))
      }
    })()
  }

  const toggleAllRootDocuments = () => {
    if (rootDocuments.length === 0) return
    onToggleAllInGroup?.(rootDocuments)
  }

  if (nodes.length === 0 && folders.length === 0) return null

  return (
    <section
      className="uploaded-library"
      aria-label={t(filterMode ? 'library.tree.filterAriaLabel' : 'library.tree.organizationAriaLabel')}
    >
      <div className="uploaded-library-toolbar">
        <button
          className="uploaded-library-heading"
          type="button"
          aria-expanded={!rootCollapsed}
          onClick={() => setRootCollapsed((value) => !value)}
        >
          <span className={'toggle-arrow ' + (rootCollapsed ? '' : 'open')}>&#9662;</span>
          <span className="uploaded-library-title">{t('library.groups.userUploads')}</span>
          <span className="uploaded-library-count">({documents.length.toLocaleString(locale)})</span>
        </button>
        {filterMode ? (
          <button
            className="uploaded-library-edit-btn"
            type="button"
            disabled={rootDocuments.length === 0}
            onClick={toggleAllRootDocuments}
          >
            {allRootSelected ? t('common.deselectAll') : t('common.selectAll')}
          </button>
        ) : (
          <button
            className="uploaded-library-edit-btn"
            type="button"
            disabled={busy}
            aria-pressed={editMode}
            onClick={() => {
              setEditMode((value) => !value)
              setSelectedKeys(new Set())
              setFolderDialog(null)
              setFolderDialogError('')
              setDeleteInfoOpen(false)
            }}
          >
            {editMode ? t('library.tree.finishEditing') : t('library.tree.organize')}
          </button>
        )}
      </div>

      {!rootCollapsed && organizing && (
        <div className="uploaded-library-actions" aria-label={t('library.tree.editActionsAriaLabel')}>
          <div className="uploaded-library-action-group">
            <span className="uploaded-library-action-label">{t('library.tree.folders')}</span>
            <div className="uploaded-library-action-row">
              <button type="button" disabled={busy} onClick={() => openFolderDialog(null)}>
                {t('library.tree.newFolder')}
              </button>
              <button
                type="button"
                disabled={busy || !selectedSingleFolder}
                onClick={renameSelectedFolder}
              >
                {t('library.tree.rename')}
              </button>
              <span className="uploaded-library-delete-control" title={deleteFolderHelp}>
                <button
                  type="button"
                  className={'uploaded-library-delete-btn' + (deleteFolderBlocked ? ' uploaded-library-delete-btn-blocked' : '')}
                  disabled={busy || (!canDeleteSelectedFolder && !deleteFolderBlocked)}
                  aria-disabled={deleteFolderBlocked}
                  aria-expanded={deleteFolderBlocked ? deleteInfoOpen : undefined}
                  aria-controls={deleteFolderBlocked ? 'uploaded-library-delete-info' : undefined}
                  onClick={deleteSelectedFolder}
                >
                  {t('library.tree.deleteFolder')}
                  {deleteFolderBlocked && <span className="uploaded-library-warning-icon" aria-hidden="true">!</span>}
                </button>
                {deleteInfoOpen && (
                  <span
                    id="uploaded-library-delete-info"
                    className="uploaded-library-info-popover"
                    role="tooltip"
                  >
                    <strong>{t('library.tree.folderNotEmpty')}</strong>
                    <span>{t('library.tree.moveContentsFirst')}</span>
                  </span>
                )}
              </span>
            </div>
          </div>
          <div className="uploaded-library-action-group uploaded-library-action-group-move">
            <label className="uploaded-library-move">
              <span className="uploaded-library-action-label">{t('library.tree.moveDocuments')}</span>
              <select
                disabled={busy || !canMoveDocuments}
                defaultValue=""
                onChange={(event) => {
                  const value = event.target.value
                  if (!value) return
                  event.target.value = ''
                  moveSelectedDocuments(value === 'root' ? null : value)
                }}
              >
                <option value="">
                  {hasMixedSelection
                    ? t('library.tree.selectDocumentsOnly')
                    : selectedDocumentIds.length > 0
                      ? t('library.tree.selectedCount', { count: selectedDocumentIds.length })
                      : t('library.tree.selectDocumentsFirst')}
                </option>
                <option value="root">{t('library.tree.root')}</option>
                {folderOptions.map((folder) => (
                  <option key={folder.id} value={folder.id}>
                    {folder.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {actionError && (
            <p className="uploaded-library-action-error" role="alert">
              {actionError}
            </p>
          )}
        </div>
      )}

      {!rootCollapsed && (
        <Tree
          aria-label={t('library.tree.uploadedDocumentsAriaLabel')}
          className="uploaded-library-tree"
          keyboardNavigationBehavior="tab"
          selectionMode="none"
          expandedKeys={expandedKeys}
          onExpandedChange={setExpandedKeys}
          onAction={handleAction}
          disabledKeys={documentOpening ? Array.from(nodeByKey.keys()) : undefined}
          renderEmptyState={() => <p className="uploaded-library-empty">{t('library.tree.noMatches')}</p>}
        >
          {nodes.map((node) => renderNode(node, {
            documentOpening,
            editMode,
            filterMode,
            expandedKeys,
            onDeleteDocument,
            onToggleFolderExpanded: toggleFolderExpanded,
            onToggleAllInGroup,
            onToggleFilter,
            onToggleSelection: toggleSelection,
            onViewDocument,
            openingDocumentUrl,
            selectedFilters,
            selectedKeys,
            locale,
            t,
            openFolderDialog,
          }))}
        </Tree>
      )}
      {folderDialog && (
        <TextInputDialog
          title={folderDialog.kind === 'rename'
            ? t('library.folderDialog.renameTitle')
            : folderDialog.parentName
              ? t('library.folderDialog.newSubfolderTitle')
              : t('library.folderDialog.newFolderTitle')}
          label={folderDialog.kind === 'create' && folderDialog.parentName
            ? t('library.folderDialog.subfolderName')
            : t('library.folderDialog.folderName')}
          description={folderDialog.kind === 'create' && folderDialog.parentName
            ? (
              <Trans
                i18nKey="library.folderDialog.inside"
                values={{ name: folderDialog.parentName }}
                components={{ name: <bdi /> }}
              />
            )
            : undefined}
          initialValue={folderDialog.kind === 'rename' ? folderDialog.initialName : ''}
          confirmLabel={folderDialog.kind === 'rename' ? t('library.tree.rename') : t('common.create')}
          busy={busy}
          error={folderDialogError}
          onCancel={() => {
            setFolderDialog(null)
            setFolderDialogError('')
          }}
          onSubmit={submitFolderDialog}
        />
      )}
      {libraryConfirmationDialog}
    </section>
  )
}

interface RenderNodeOptions {
  documentOpening: boolean
  editMode: boolean
  filterMode: boolean
  expandedKeys: Set<Key>
  locale: string
  openingDocumentUrl?: string
  onDeleteDocument?: (doc: DocumentInfo) => Promise<void> | void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void
  onToggleFilter?: (url: string) => void
  onToggleFolderExpanded: (key: string) => void
  onToggleSelection: (key: string) => void
  onViewDocument?: (url: string) => void
  selectedFilters?: Set<string>
  selectedKeys: Set<Key>
  t: TFunction
  openFolderDialog: (parentId: string | null, parentName?: string) => void
}

/**
 * Render one React Aria tree node for both Library and Search filter modes.
 *
 * The mode branches are kept here so the tree shape stays shared while browse,
 * organize, and filter actions remain visually scoped to their owning surfaces.
 */
function renderNode(node: LibraryNode, options: RenderNodeOptions): ReactNode {
  const opening = node.kind === 'document' && options.openingDocumentUrl === node.url
  const expanded = node.kind === 'folder' && options.expandedKeys.has(node.key)
  const filterDocuments = node.kind === 'folder' ? collectDocuments(node) : []
  const folderFilterSelected = filterDocuments.length > 0 && filterDocuments.every((doc) => options.selectedFilters?.has(doc.url))
  const documentFilterSelected = node.kind === 'document' && Boolean(options.selectedFilters?.has(node.url))
  const selected = options.selectedKeys.has(node.key) || folderFilterSelected || documentFilterSelected
  const className = [
    'uploaded-library-item',
    'uploaded-library-' + node.kind,
    selected ? 'uploaded-library-item-selected' : '',
    opening ? 'uploaded-library-item-opening' : '',
  ].filter(Boolean).join(' ')
  return (
    <TreeItem
      key={node.key}
      id={node.key}
      textValue={node.title}
      className={className}
    >
      <TreeItemContent>
        <div className="uploaded-library-content">
          {node.kind === 'folder' && (
            <Button slot="chevron" className="uploaded-library-chevron">
              &#9656;
            </Button>
          )}
          <div className="uploaded-library-row">
            {options.editMode && (
              <input
                className="uploaded-library-select"
                type="checkbox"
                checked={options.selectedKeys.has(node.key)}
                aria-label={options.t('library.tree.select', { title: node.title })}
                onClick={(event) => event.stopPropagation()}
                onChange={(event) => {
                  event.stopPropagation()
                  options.onToggleSelection(node.key)
                }}
              />
            )}
            {options.filterMode && node.kind === 'folder' && (
              <input
                className="uploaded-library-select"
                type="checkbox"
                checked={folderFilterSelected}
                disabled={filterDocuments.length === 0}
                aria-label={options.t('library.tree.selectAllIn', { title: node.title })}
                onClick={(event) => event.stopPropagation()}
                onChange={(event) => {
                  event.stopPropagation()
                  options.onToggleAllInGroup?.(filterDocuments)
                }}
              />
            )}
            {options.filterMode && node.kind === 'document' && (
              <input
                className="uploaded-library-select"
                type="checkbox"
                checked={documentFilterSelected}
                aria-label={options.t('library.tree.filterBy', { title: node.title })}
                onClick={(event) => event.stopPropagation()}
                onChange={(event) => {
                  event.stopPropagation()
                  options.onToggleFilter?.(node.url)
                }}
              />
            )}
            {node.kind === 'folder' ? (
              <button
                className="uploaded-library-name uploaded-library-name-button"
                type="button"
                aria-expanded={expanded}
                onClick={(event) => {
                  event.stopPropagation()
                  options.onToggleFolderExpanded(node.key)
                }}
              >
                <bdi>{node.title}</bdi>{' '}
                <span className="uploaded-library-folder-count">
                  ({node.documentCount.toLocaleString(options.locale)})
                </span>
              </button>
            ) : (
              <bdi className="uploaded-library-name">{node.title}</bdi>
            )}
            {opening && <span className="uploaded-library-opening">{options.t('common.opening')}</span>}
            {node.kind === 'document' && !options.editMode && !options.filterMode && (
              <button
                className="document-row-action document-row-action-view"
                type="button"
                disabled={options.documentOpening}
                onClick={(event) => {
                  event.stopPropagation()
                  if (!options.documentOpening) options.onViewDocument?.(node.url)
                }}
              >
                {opening ? options.t('common.opening') : options.t('common.view')}
              </button>
            )}
            {node.kind === 'document' && options.editMode && !options.filterMode && options.onDeleteDocument && (
              <button
                className="document-row-action document-row-action-danger"
                type="button"
                disabled={options.documentOpening}
                onClick={(event) => {
                  event.stopPropagation()
                  if (!options.documentOpening) void options.onDeleteDocument?.(node.doc)
                }}
              >
                {options.t('common.delete')}
              </button>
            )}
            {options.editMode && !options.filterMode && node.kind === 'folder' && node.depth < 4 && (
              <button
                className="document-row-action document-row-action-secondary"
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  options.openFolderDialog(node.id, node.title)
                }}
              >
                {options.t('library.tree.newSubfolder')}
              </button>
            )}
          </div>
        </div>
      </TreeItemContent>
      {node.children.map((child) => renderNode(child, options))}
    </TreeItem>
  )
}

/**
 * Merge uploaded-document metadata with folder locations into one render tree.
 *
 * The backend stores folders and document locations separately; this function
 * joins them in memory so Library and Search can share identical hierarchy.
 */
function buildLibraryTree(
  documents: DocumentInfo[],
  organization: UploadedLibraryOrganization,
  options: { hideEmptyFolders?: boolean; locale?: string } = {},
): {
  nodes: LibraryNode[]
  folders: UploadedLibraryFolder[]
  folderOptions: { id: string; label: string }[]
  nodeByKey: Map<string, LibraryNode>
} {
  const collator = new Intl.Collator(options.locale, { numeric: true, sensitivity: 'base' })
  const uploadDocs = documents
    .filter((doc) => doc.source === 'upload' && isUploadedDocumentUrl(doc.url))
    .map((doc) => ({ ...doc, uploadId: uploadIdFromUrl(doc.url) }))
    .filter((doc): doc is DocumentInfo & { uploadId: string } => Boolean(doc.uploadId))
  const foldersByParent = groupFoldersByParent(organization.folders)
  const locations = new Map(organization.documentLocations.map((location) => [location.documentId, location]))
  const docsByFolder = new Map<string, (DocumentInfo & { uploadId: string })[]>()
  for (const doc of uploadDocs) {
    const folderId = locations.get(doc.uploadId)?.folderId ?? null
    const key = folderId ?? ''
    const list = docsByFolder.get(key)
    if (list) list.push(doc)
    else docsByFolder.set(key, [doc])
  }

  const nodeByKey = new Map<string, LibraryNode>()
  const buildFolder = (folder: UploadedLibraryFolder): LibraryNode | null => {
    const children = [
      ...sortFolders(foldersByParent.get(folder.id) ?? [], collator)
        .map(buildFolder)
        .filter((node): node is LibraryNode => Boolean(node)),
      ...sortDocuments(docsByFolder.get(folder.id) ?? [], locations, collator).map(documentNode),
    ]
    if (options.hideEmptyFolders && countDocuments(children) === 0) return null
    const node: LibraryNode = {
      key: folderKey(folder.id),
      kind: 'folder',
      id: folder.id,
      title: folder.name,
      depth: folder.depth,
      documentCount: countDocuments(children),
      children,
    }
    nodeByKey.set(node.key, node)
    return node
  }
  const documentNode = (doc: DocumentInfo & { uploadId: string }): LibraryNode => {
    const node: LibraryNode = {
      key: documentKey(doc.uploadId),
      kind: 'document',
      id: doc.uploadId,
      title: doc.title,
      url: doc.url,
      doc,
      children: [],
    }
    nodeByKey.set(node.key, node)
    return node
  }

  const nodes = [
    ...sortFolders(foldersByParent.get('') ?? [], collator)
      .map(buildFolder)
      .filter((node): node is LibraryNode => Boolean(node)),
    ...sortDocuments(docsByFolder.get('') ?? [], locations, collator).map(documentNode),
  ]
  return {
    nodes,
    folders: organization.folders,
    folderOptions: buildFolderOptions(organization.folders, collator),
    nodeByKey,
  }
}

/** Collect descendant documents for folder-level search filter toggles. */
function collectDocuments(node: LibraryNode): DocumentInfo[] {
  if (node.kind === 'document') return [node.doc]
  return node.children.flatMap(collectDocuments)
}

/** Group folders by parent id, using an empty string as the root bucket key. */
function groupFoldersByParent(folders: UploadedLibraryFolder[]): Map<string, UploadedLibraryFolder[]> {
  const groups = new Map<string, UploadedLibraryFolder[]>()
  for (const folder of folders) {
    const key = folder.parentId ?? ''
    const list = groups.get(key)
    if (list) list.push(folder)
    else groups.set(key, [folder])
  }
  return groups
}

/** Count only document descendants, not folders, for folder count badges. */
function countDocuments(nodes: LibraryNode[]): number {
  return nodes.reduce((total, node) => (
    total + (node.kind === 'document' ? 1 : countDocuments(node.children))
  ), 0)
}

/** Preserve manual folder order, with name as a stable tie-breaker. */
function sortFolders(folders: UploadedLibraryFolder[], collator: Intl.Collator): UploadedLibraryFolder[] {
  return folders.slice().sort((a, b) => a.sortOrder - b.sortOrder || collator.compare(a.name, b.name))
}

/** Preserve manual document order, with title as a stable tie-breaker. */
function sortDocuments(
  docs: (DocumentInfo & { uploadId: string })[],
  locations: Map<string, { sortOrder: number }>,
  collator: Intl.Collator,
): (DocumentInfo & { uploadId: string })[] {
  return docs
    .slice()
    .sort((a, b) => (
      (locations.get(a.uploadId)?.sortOrder ?? 0) - (locations.get(b.uploadId)?.sortOrder ?? 0) ||
      collator.compare(a.title, b.title)
    ))
}

/** Build move-target labels that include parent path context for duplicate names elsewhere. */
function buildFolderOptions(
  folders: UploadedLibraryFolder[],
  collator: Intl.Collator,
): { id: string; label: string }[] {
  const byId = new Map(folders.map((folder) => [folder.id, folder]))
  return sortFolders(folders, collator).map((folder) => ({
    id: folder.id,
    label: folderPath(folder, byId),
  }))
}

/** Walk parent pointers to display a folder path like `Parent / Child`. */
function folderPath(folder: UploadedLibraryFolder, byId: Map<string, UploadedLibraryFolder>): string {
  const parts = [folder.name]
  let parentId = folder.parentId
  while (parentId) {
    const parent = byId.get(parentId)
    if (!parent) break
    parts.unshift(parent.name)
    parentId = parent.parentId
  }
  return parts.join(' / ')
}

/** Extract the backend upload id from the app-local uploaded document URL. */
function uploadIdFromUrl(url: string): string | null {
  const match = url.match(/^\/uploads\/([a-fA-F0-9]+)\.html(?:[#?].*)?$/)
  return match?.[1] ?? null
}

/** Prefix ids so React Aria keys cannot collide between folders and documents. */
function folderKey(id: string): string {
  return 'folder:' + id
}

/** Prefix ids so React Aria keys cannot collide between documents and folders. */
function documentKey(id: string): string {
  return 'document:' + id
}
