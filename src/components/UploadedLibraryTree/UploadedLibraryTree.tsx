import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Button, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import {
  type UploadedDocumentDeleteBatchResult,
  type UploadedLibraryOrganization,
} from '../../uploads/DocumentUploads'
import { useAppConfirmation } from '../AppDialog/useAppConfirmation'
import { TextInputDialog } from '../TextInputDialog/TextInputDialog'
import { buildLibraryTree, collectDocuments, type LibraryNode } from './libraryTree'
import './UploadedLibraryTree.css'

interface UploadedLibraryTreeProps {
  documents: DocumentInfo[]
  organization: UploadedLibraryOrganization
  mode?: 'library' | 'filter'
  documentOpening?: boolean
  mutationDisabled?: boolean
  resetEditing?: boolean
  openingDocumentUrl?: string
  selectedFilters?: Set<string>
  onCreateFolder?: (parentId: string | null, name: string) => Promise<void> | void
  onDeleteDocuments?: (docs: DocumentInfo[]) => Promise<UploadedDocumentDeleteBatchResult | null>
  onDeleteFolder?: (folderId: string) => Promise<void> | void
  onMoveDocuments?: (documentIds: string[], folderId: string | null) => Promise<void> | void
  onRenameFolder?: (folderId: string, name: string) => Promise<void> | void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void
  onToggleFilter?: (url: string) => void
  onViewDocument?: (url: string) => void
}

type FolderDialogState =
  | { kind: 'create'; parentId: string | null; parentName?: string }
  | { kind: 'rename'; folderId: string; initialName: string }

export function UploadedLibraryTree({
  documents,
  organization,
  mode = 'library',
  documentOpening = false,
  mutationDisabled = false,
  resetEditing = false,
  openingDocumentUrl,
  selectedFilters,
  onCreateFolder,
  onDeleteFolder,
  onDeleteDocuments,
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
  const documentNodes = useMemo(
    () => Array.from(nodeByKey.values()).filter((node): node is Extract<LibraryNode, { kind: 'document' }> => node.kind === 'document'),
    [nodeByKey],
  )
  const allRootSelected = rootDocuments.length > 0 && rootDocuments.every((doc) => selectedFilters?.has(doc.url))
  const selectedNodes = Array.from(selectedKeys)
    .map((key) => nodeByKey.get(String(key)))
    .filter((node): node is LibraryNode => Boolean(node))
  const selectedDocuments = selectedNodes.filter(
    (node): node is Extract<LibraryNode, { kind: 'document' }> => node.kind === 'document',
  )
  const selectedDocumentIds = selectedDocuments.map((node) => node.id)
  const selectedFolders = selectedNodes.filter((node) => node.kind === 'folder')
  const canMoveDocuments = organizing && !mutationDisabled && selectedDocumentIds.length > 0 && selectedFolders.length === 0
  const canDeleteDocuments = canMoveDocuments && !busy && Boolean(onDeleteDocuments)
  const allDocumentsSelected = documentNodes.length > 0 && documentNodes.every((node) => selectedKeys.has(node.key))
  const selectedSingleFolder = selectedFolders.length === 1 && selectedDocumentIds.length === 0
  const selectedFolder = selectedSingleFolder ? selectedFolders[0] : undefined
  const selectedFolderHasContents = Boolean(selectedFolder && (
    selectedFolder.documentCount > 0 || selectedFolder.children.length > 0
  ))
  const canDeleteSelectedFolder = Boolean(selectedFolder && !selectedFolderHasContents && !busy && !mutationDisabled)
  const deleteFolderBlocked = Boolean(selectedFolderHasContents && !busy && !mutationDisabled)
  const deleteFolderHelp = !selectedSingleFolder
    ? t('library.tree.selectOneFolder')
    : selectedFolderHasContents
      ? t('library.tree.moveOrRemoveContents')
      : t('library.tree.deleteSelectedFolder')

  useEffect(() => {
    setDeleteInfoOpen(false)
    setActionError('')
  }, [selectedKeys, editMode])

  // A batch import owns the shared library mutation slot, so leave manage
  // mode and close any pending folder edit before new documents arrive.
  useEffect(() => {
    if (!resetEditing) return
    setEditMode(false)
    setSelectedKeys(new Set())
    setFolderDialog(null)
    setDeleteInfoOpen(false)
  }, [resetEditing])

  const runEditAction = async (action: () => Promise<void> | void) => {
    if (mutationDisabled) return
    setBusy(true)
    try {
      await action()
      setSelectedKeys(new Set())
    } finally {
      setBusy(false)
    }
  }

  const toggleSelection = (key: string) => {
    const node = nodeByKey.get(key)
    if (!node) return
    setSelectedKeys((current) => {
      if (current.has(key)) {
        const next = new Set(current)
        next.delete(key)
        return next
      }
      if (node.kind === 'folder') return new Set([key])

      // Document and folder actions have different semantics, so selecting a
      // document drops any folder selection instead of permitting a mixed state.
      const next = new Set(Array.from(current).filter((selectedKey) => (
        nodeByKey.get(String(selectedKey))?.kind === 'document'
      )))
      next.add(key)
      return next
    })
  }

  const selectAllDocuments = () => {
    setSelectedKeys(new Set(documentNodes.map((node) => node.key)))
  }

  const clearSelection = () => {
    setSelectedKeys(new Set())
  }

  const deleteSelectedDocuments = () => {
    if (!canDeleteDocuments || !onDeleteDocuments) return
    const documentsToDelete = selectedDocuments.map((node) => node.doc)
    setActionError('')
    void (async () => {
      const confirmed = await confirmLibraryAction({
        title: t('library.confirmDeleteDocuments.title'),
        description: t('library.confirmDeleteDocuments.description'),
        details: [{
          label: t('library.confirmDeleteDocuments.count'),
          value: documentsToDelete.length.toLocaleString(locale),
        }],
        confirmLabel: t('library.confirmDeleteDocuments.confirm', { count: documentsToDelete.length }),
        tone: 'danger',
      })
      if (!confirmed) return

      setBusy(true)
      try {
        const result = await onDeleteDocuments(documentsToDelete)
        if (!result) return
        const failedUrls = new Set(result.failures.map((failure) => failure.documentUrl))
        setSelectedKeys(new Set(
          selectedDocuments
            .filter((node) => failedUrls.has(node.url))
            .map((node) => node.key),
        ))
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err))
      } finally {
        setBusy(false)
      }
    })()
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
    if (documentOpening) return
    const node = nodeByKey.get(String(key))
    if (!node) return
    if (filterMode) {
      if (node.kind === 'folder') onToggleAllInGroup?.(collectDocuments(node))
      else onToggleFilter?.(node.url)
      return
    }
    if (editMode) {
      toggleSelection(String(key))
      return
    }
    if (node.kind === 'folder') {
      toggleFolderExpanded(node.key)
      return
    }
    if (node.kind === 'document') onViewDocument?.(node.url)
  }

  const openFolderDialog = (parentId: string | null, parentName?: string) => {
    setFolderDialogError('')
    setActionError('')
    setDeleteInfoOpen(false)
    setFolderDialog({ kind: 'create', parentId, parentName })
  }

  const submitFolderDialog = (name: string) => {
    if (!folderDialog || mutationDisabled) return
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
          <div className="uploaded-library-toolbar-actions">
            {organizing && (
              <button
                className="uploaded-library-edit-btn"
                type="button"
                disabled={busy || mutationDisabled}
                onClick={() => openFolderDialog(null)}
              >
                {t('library.tree.newFolder')}
              </button>
            )}
            <button
              className="uploaded-library-edit-btn"
              type="button"
              disabled={busy || mutationDisabled}
              aria-pressed={editMode}
              onClick={() => {
                setEditMode((value) => !value)
                setSelectedKeys(new Set())
                setFolderDialog(null)
                setFolderDialogError('')
                setDeleteInfoOpen(false)
              }}
            >
              {editMode ? t('common.done') : t('library.tree.organize')}
            </button>
          </div>
        )}
      </div>

      {!rootCollapsed && organizing && (
        <div
          className="uploaded-library-actions"
          role="group"
          aria-label={t('library.tree.editActionsAriaLabel')}
        >
          <div className="uploaded-library-context-actions">
            <div className="uploaded-library-selection-actions">
              <strong className="uploaded-library-selection-count" aria-live="polite">
                {selectedFolder
                  ? t('library.tree.folderSelected')
                  : t('library.tree.selectedCount', { count: selectedDocumentIds.length })}
              </strong>
              {!selectedFolder && selectedDocumentIds.length === 0 && (
                <button
                  type="button"
                  disabled={busy || mutationDisabled || allDocumentsSelected || documentNodes.length === 0}
                  onClick={selectAllDocuments}
                >
                  {t('common.selectAll')}
                </button>
              )}
              {selectedKeys.size > 0 && (
                <button
                  type="button"
                  disabled={busy || mutationDisabled}
                  onClick={clearSelection}
                >
                  {t('common.deselectAll')}
                </button>
              )}
            </div>
            {selectedDocumentIds.length > 0 && (
              <div className="uploaded-library-task-actions">
                <label className="uploaded-library-move">
                  <span className="uploaded-library-action-label">{t('library.tree.moveDocuments')}</span>
                  <select
                    disabled={busy || mutationDisabled || !canMoveDocuments}
                    defaultValue=""
                    onChange={(event) => {
                      const value = event.target.value
                      if (!value) return
                      event.target.value = ''
                      moveSelectedDocuments(value === 'root' ? null : value)
                    }}
                  >
                    <option value="">{t('library.tree.chooseDestination')}</option>
                    <option value="root">{t('library.tree.root')}</option>
                    {folderOptions.map((folder) => (
                      <option key={folder.id} value={folder.id}>
                        {folder.label}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  className="uploaded-library-batch-delete"
                  disabled={!canDeleteDocuments}
                  onClick={deleteSelectedDocuments}
                >
                  {t('common.delete')}
                </button>
              </div>
            )}
            {selectedFolder && (
              <div className="uploaded-library-task-actions">
                <button
                  type="button"
                  disabled={busy || mutationDisabled || !selectedSingleFolder}
                  onClick={renameSelectedFolder}
                >
                  {t('library.tree.rename')}
                </button>
                <span className="uploaded-library-delete-control" title={deleteFolderHelp}>
                  <button
                    type="button"
                    className={'uploaded-library-delete-btn' + (deleteFolderBlocked ? ' uploaded-library-delete-btn-blocked' : '')}
                    disabled={busy || mutationDisabled || (!canDeleteSelectedFolder && !deleteFolderBlocked)}
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
            )}
          </div>
          {actionError && (
            <p className="uploaded-library-action-error" role="alert" dir="auto">
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
  const selectionEnabled = options.editMode || options.filterMode
  const selectionDisabled = options.filterMode && node.kind === 'folder' && filterDocuments.length === 0
  const selectionLabel = options.editMode
    ? options.t('library.tree.select', { title: node.title })
    : node.kind === 'folder'
      ? options.t('library.tree.selectAllIn', { title: node.title })
      : options.t('library.tree.filterBy', { title: node.title })
  const toggleNodeSelection = () => {
    if (options.editMode) options.onToggleSelection(node.key)
    else if (node.kind === 'folder') options.onToggleAllInGroup?.(filterDocuments)
    else options.onToggleFilter?.(node.url)
  }
  const className = [
    'uploaded-library-item',
    'uploaded-library-' + node.kind,
    selectionEnabled || node.kind === 'folder' ? 'uploaded-library-item-actionable' : '',
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
            {selectionEnabled ? (
              <span className="uploaded-library-name uploaded-library-selection">
                <input
                  className="uploaded-library-select"
                  type="checkbox"
                  checked={selected}
                  disabled={selectionDisabled}
                  aria-label={selectionLabel}
                  onClick={(event) => event.stopPropagation()}
                  onChange={(event) => {
                    event.stopPropagation()
                    toggleNodeSelection()
                  }}
                />
                <span className="uploaded-library-selection-text">
                  <bdi>{node.title}</bdi>
                  {node.kind === 'folder' && (
                    <>
                      {' '}
                      <span className="uploaded-library-folder-count">
                        ({node.documentCount.toLocaleString(options.locale)})
                      </span>
                    </>
                  )}
                </span>
              </span>
            ) : node.kind === 'folder' ? (
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
