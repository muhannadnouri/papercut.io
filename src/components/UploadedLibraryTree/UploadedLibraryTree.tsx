import { MoreActionsIcon } from '../icons'
import {
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import {
  Trans,
  useTranslation,
} from 'react-i18next'
import type { TFunction } from 'i18next'
import {
  Button,
  Menu,
  MenuItem,
  MenuTrigger,
  Popover,
  Tree,
  TreeItem,
  TreeItemContent,
  type Key,
} from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import {
  type UploadedDocumentDeleteBatchResult,
  type UploadedLibraryOrganization,
} from '../../uploads/DocumentUploads'
import { useAppConfirmation } from '../AppDialog/useAppConfirmation'
import { TrashIcon } from '../TrashIcon/TrashIcon'
import { ChevronIcon } from '../ChevronIcon/ChevronIcon'
import { TextInputDialog } from '../TextInputDialog/TextInputDialog'
import {
  buildLibraryTree,
  collectDocuments,
  countDescendantFolders,
  type LibraryNode,
} from './libraryTree'
import './UploadedLibraryTree.css'

interface UploadedLibraryTreeProps {
  documents: DocumentInfo[]
  unfilteredDocuments?: DocumentInfo[]
  organization: UploadedLibraryOrganization
  mode?: 'library' | 'filter'
  filterActive?: boolean
  documentOpening?: boolean
  mutationDisabled?: boolean
  onDone?: () => void
  openingDocumentUrl?: string
  savedAudiobookDocumentUrls?: ReadonlySet<string>
  /** Existing Manage operation status, placed here so sticky controls keep it visible. */
  managementStatus?: ReactNode
  selectedFilters?: Set<string>
  onCreateFolder?: (parentId: string | null, name: string) => Promise<void> | void
  onDeleteDocuments?: (docs: DocumentInfo[]) => Promise<UploadedDocumentDeleteBatchResult | null>
  onDeleteFolder?: (folderId: string) => Promise<UploadedDocumentDeleteBatchResult | null>
  onMoveDocuments?: (documentIds: string[], folderId: string | null) => Promise<void> | void
  onRenameFolder?: (folderId: string, name: string) => Promise<void> | void
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void
  onToggleFilter?: (url: string) => void
  onViewAudiobooks?: () => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onViewDocument?: (url: string) => void
}

type FolderDialogState =
  | { kind: 'create';
    parentId: string | null;
    parentName?: string }
  | { kind: 'rename';
    folderId: string;
    initialName: string }

export function UploadedLibraryTree({
  documents,
  unfilteredDocuments = documents,
  organization,
  mode = 'library',
  filterActive = false,
  documentOpening = false,
  mutationDisabled = false,
  onDone,
  openingDocumentUrl,
  savedAudiobookDocumentUrls = new Set(),
  managementStatus,
  selectedFilters,
  onCreateFolder,
  onDeleteFolder,
  onDeleteDocuments,
  onMoveDocuments,
  onRenameFolder,
  onToggleAllInGroup,
  onToggleFilter,
  onViewAudiobooks,
  onViewDocumentInfo,
  onViewDocument,
}: UploadedLibraryTreeProps) {
  const { t, i18n } = useTranslation()
  const [selectedUrls, setSelectedUrls] = useState<Set<string>>(new Set())
  const [expandedKeys, setExpandedKeys] = useState<Set<Key>>(new Set())
  const [rootCollapsed, setRootCollapsed] = useState(false)
  const [folderDialog, setFolderDialog] = useState<FolderDialogState | null>(null)
  const [folderDialogError, setFolderDialogError] = useState('')
  const [actionError, setActionError] = useState('')
  const [busy, setBusy] = useState(false)
  const { confirm: confirmLibraryAction, dialog: libraryConfirmationDialog } = useAppConfirmation()
  const filterMode = mode === 'filter'
  const editMode = Boolean(onDone)
  const organizing = mode === 'library' && editMode
  const locale = i18n.resolvedLanguage ?? i18n.language
  const { nodes, folders, nodeByKey, folderOptions } = useMemo(
    () => buildLibraryTree(documents, organization, {
      hideEmptyFolders: filterMode || filterActive,
      locale,
    }),
    [documents, filterActive, filterMode, locale, organization],
  )
  // Destructive counts must include documents hidden by active Library filters.
  const unfilteredNodeByKey = useMemo(
    () => buildLibraryTree(unfilteredDocuments, organization, { locale }).nodeByKey,
    [locale, organization, unfilteredDocuments],
  )
  const rootDocuments = useMemo(() => nodes.flatMap(collectDocuments), [nodes])
  const documentNodes = useMemo(
    () => Array.from(nodeByKey.values()).filter((node): node is Extract<LibraryNode, { kind: 'document' }> => node.kind === 'document'),
    [nodeByKey],
  )
  const visibleExpandedKeys = filterActive
    ? new Set(Array.from(nodeByKey.values()).filter((node) => node.kind === 'folder').map((node) => node.key))
    : expandedKeys
  const allRootSelected = rootDocuments.length > 0 && rootDocuments.every((doc) => selectedFilters?.has(doc.url))
  const selectedDocuments = documentNodes.filter(node => selectedUrls.has(node.url))
  const selectedDocumentIds = selectedDocuments.map(node => node.id)
  const canMoveDocuments = organizing && !mutationDisabled && selectedDocumentIds.length > 0
  const canDeleteDocuments = canMoveDocuments && !busy && Boolean(onDeleteDocuments)
  const allDocumentsSelected = documentNodes.length > 0 && selectedDocuments.length === documentNodes.length

  useEffect(() => {
    setActionError('')
  }, [selectedUrls, editMode])

  useEffect(() => {
    if (editMode) {
      setRootCollapsed(false)
    } else {
      setSelectedUrls(new Set())
      setFolderDialog(null)
      setFolderDialogError('')
    }
  }, [editMode])

  const runEditAction = async (action: () => Promise<void> | void) => {
    if (mutationDisabled) {
      return
    }
    setBusy(true)
    try {
      await action()
    } finally {
      setBusy(false)
    }
  }

  // The displayed tree contains matching documents even when their ancestors are collapsed.
  const toggleSelection = (documents: DocumentInfo[]) => {
    if (busy || mutationDisabled || documents.length === 0) {
      return
    }
    setSelectedUrls(current => {
      const deselect = documents.every(doc => current.has(doc.url))
      const next = new Set(current)
      for (const doc of documents) {
        if (deselect) {
          next.delete(doc.url)
        } else {
          next.add(doc.url)
        }
      }
      return next
    })
  }

  const selectAllDocuments = () => {
    setSelectedUrls(new Set(documentNodes.map(node => node.url)))
  }

  const clearSelection = () => {
    setSelectedUrls(new Set())
  }

  async function deletionBlockedBySavedAudio(documents: DocumentInfo[], kind: 'batch' | 'folder') {
    if (!documents.some(doc => savedAudiobookDocumentUrls.has(doc.url))) {
      return false
    }
    const viewAudiobooks = await confirmLibraryAction({
      title: t(kind === 'folder' ? 'library.savedAudioDependency.folderTitle' : 'library.savedAudioDependency.batchTitle'),
      description: t(kind === 'folder' ? 'library.savedAudioDependency.folderDescription' : 'library.savedAudioDependency.batchDescription'),
      confirmLabel: t('library.savedAudioDependency.viewAudiobooks'),
    })
    if (viewAudiobooks) {
      onViewAudiobooks?.()
    }
    return true
  }

  const deleteSelectedDocuments = () => {
    if (!canDeleteDocuments || !onDeleteDocuments) {
      return
    }
    const documentsToDelete = selectedDocuments.map((node) => node.doc)
    setActionError('')
    void (async () => {
      if (await deletionBlockedBySavedAudio(documentsToDelete, 'batch')) {
        return
      }

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
      if (!confirmed) {
        return
      }

      setBusy(true)
      try {
        const result = await onDeleteDocuments(documentsToDelete)
        if (!result) {
          return
        }
        const failedUrls = new Set(result.failures.map((failure) => failure.documentUrl))
        setSelectedUrls(new Set(
          selectedDocuments
            .filter((node) => failedUrls.has(node.url))
            .map((node) => node.url),
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
      if (next.has(key)) {
        next.delete(key)
      } else {
        next.add(key)
      }
      return next
    })
  }

  const handleAction = (key: Key) => {
    if (documentOpening) {
      return
    }
    const node = nodeByKey.get(String(key))
    if (!node) {
      return
    }
    if (filterMode) {
      if (node.kind === 'folder') {
        onToggleAllInGroup?.(collectDocuments(node))
      } else {
        onToggleFilter?.(node.url)
      }
      return
    }
    if (editMode) {
      toggleSelection(collectDocuments(node))
      return
    }
    if (node.kind === 'folder') {
      toggleFolderExpanded(node.key)
      return
    }
    if (node.kind === 'document') {
      onViewDocument?.(node.url)
    }
  }

  const openFolderDialog = (parentId: string | null, parentName?: string) => {
    setFolderDialogError('')
    setActionError('')
    setFolderDialog({
      kind: 'create',
      parentId,
      parentName,
    })
  }

  const submitFolderDialog = (name: string) => {
    if (!folderDialog || mutationDisabled) {
      return
    }
    const target = folderDialog
    void (async () => {
      try {
        if (target.kind === 'create') {
          if (!onCreateFolder) {
            return
          }
          await runEditAction(() => onCreateFolder(target.parentId, name))
        } else {
          if (!onRenameFolder) {
            return
          }
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

  const renameFolder = (folder: FolderNode) => {
    setFolderDialogError('')
    setActionError('')
    setFolderDialog({
      kind: 'rename',
      folderId: folder.id,
      initialName: folder.title,
    })
  }

  const deleteFolder = (folder: FolderNode) => {
    if (busy || mutationDisabled || !onDeleteFolder) {
      return
    }
    const fullFolder = unfilteredNodeByKey.get(folder.key)
    if (!fullFolder || fullFolder.kind !== 'folder') {
      return
    }
    const documentsToDelete = collectDocuments(fullFolder)
    const nestedFolderCount = countDescendantFolders(fullFolder)
    const hasContents = documentsToDelete.length > 0 || nestedFolderCount > 0
    setActionError('')
    void (async () => {
      if (await deletionBlockedBySavedAudio(documentsToDelete, 'folder')) {
        return
      }

      const confirmed = await confirmLibraryAction({
        title: t(hasContents
          ? 'library.confirmDeleteFolderContents.title'
          : 'library.confirmDeleteFolder.title'),
        description: t(hasContents
          ? 'library.confirmDeleteFolderContents.description'
          : 'library.confirmDeleteFolder.description'),
        details: [
          {
            label: t('library.confirmDeleteFolder.folder'),
            value: <bdi>{folder.title}</bdi>,
          },
          ...(hasContents ? [
            {
              label: t('library.confirmDeleteFolderContents.nestedFolders'),
              value: nestedFolderCount.toLocaleString(locale),
            },
            {
              label: t('library.confirmDeleteFolderContents.documents'),
              value: documentsToDelete.length.toLocaleString(locale),
            },
          ] : []),
        ],
        confirmLabel: t('library.tree.deleteFolder'),
        tone: 'danger',
      })
      if (!confirmed) {
        return
      }

      setBusy(true)
      try {
        await onDeleteFolder(folder.id)
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err))
      } finally {
        setBusy(false)
      }
    })()
  }

  const moveSelectedDocuments = (folderId: string | null) => {
    if (!canMoveDocuments) {
      return
    }
    if (!onMoveDocuments) {
      return
    }
    setActionError('')
    void (async () => {
      try {
        await runEditAction(() => onMoveDocuments(selectedDocumentIds, folderId))
        clearSelection()
      } catch (err) {
        setActionError(err instanceof Error ? err.message : String(err))
      }
    })()
  }

  const toggleAllRootDocuments = () => {
    if (rootDocuments.length === 0) {
      return
    }
    onToggleAllInGroup?.(rootDocuments)
  }

  if (nodes.length === 0 && folders.length === 0 && !managementStatus) {
    return null
  }

  return (
    <section
      className="uploaded-library"
      aria-label={t(filterMode ? 'library.tree.filterAriaLabel' : 'library.tree.organizationAriaLabel')}
    >
      <div className={(organizing && !rootCollapsed) || managementStatus
        ? 'uploaded-library-manage-header uploaded-library-manage-header-sticky'
        : 'uploaded-library-manage-header'}>
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
          ) : onDone && (
            <div className="uploaded-library-toolbar-actions">
              <button
                className="uploaded-library-edit-btn uploaded-library-icon-action"
                type="button"
                disabled={busy || mutationDisabled}
                onClick={() => openFolderDialog(null)}
              >
                <FolderIcon className="uploaded-library-action-icon" />
                {t('library.tree.newFolder')}
              </button>
              <button
                className="uploaded-library-edit-btn uploaded-library-done"
                type="button"
                disabled={busy || mutationDisabled}
                autoFocus
                onClick={onDone}
              >
                {t('common.done')}
              </button>
            </div>
          )}
        </div>

        {managementStatus}

        {!rootCollapsed && organizing && (
          <div
            className="uploaded-library-actions"
            role="group"
            aria-label={t('library.tree.editActionsAriaLabel')}
          >
            <div className="uploaded-library-context-actions">
              <div className="uploaded-library-selection-actions">
                <strong className="uploaded-library-selection-count" aria-live="polite">
                  {t('library.tree.selectedCount', { count: selectedDocumentIds.length })}
                </strong>
                <button
                  type="button"
                  disabled={busy || mutationDisabled || documentNodes.length === 0}
                  onClick={allDocumentsSelected ? clearSelection : selectAllDocuments}
                >
                  {allDocumentsSelected ? t('common.deselectAll') : t('library.tree.selectAllDocuments')}
                </button>
                {selectedDocuments.length > 0 && !allDocumentsSelected && (
                  <button type="button" disabled={busy || mutationDisabled} onClick={clearSelection}>
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
                        if (!value) {
                          return
                        }
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
                    className="uploaded-library-batch-delete uploaded-library-icon-action"
                    disabled={!canDeleteDocuments}
                    onClick={deleteSelectedDocuments}
                  >
                    <TrashIcon className="uploaded-library-action-icon" />
                    {t('common.delete')}
                  </button>
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
      </div>

      {!rootCollapsed && (
        <Tree
          aria-label={t('library.tree.uploadedDocumentsAriaLabel')}
          className="uploaded-library-tree"
          keyboardNavigationBehavior="tab"
          selectionMode="none"
          expandedKeys={visibleExpandedKeys}
          onExpandedChange={setExpandedKeys}
          onAction={handleAction}
          disabledKeys={documentOpening ? Array.from(nodeByKey.keys()) : undefined}
          renderEmptyState={() => <p className="uploaded-library-empty">{t('library.tree.noMatches')}</p>}
        >
          {nodes.map((node) => renderNode(node, {
            documentOpening,
            editMode,
            filterMode,
            expandedKeys: visibleExpandedKeys,
            onToggleFolderExpanded: toggleFolderExpanded,
            onToggleAllInGroup,
            onToggleFilter,
            onToggleSelection: toggleSelection,
            onViewDocumentInfo,
            onViewDocument,
            openingDocumentUrl,
            selectedFilters,
            selectedUrls,
            editActionsDisabled: busy || mutationDisabled,
            renameFolder,
            deleteFolder,
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

type FolderNode = Extract<LibraryNode, { kind: 'folder' }>
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
  onToggleSelection: (documents: DocumentInfo[]) => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onViewDocument?: (url: string) => void
  selectedFilters?: Set<string>
  selectedUrls: Set<string>
  editActionsDisabled: boolean
  renameFolder: (folder: FolderNode) => void
  deleteFolder: (folder: FolderNode) => void
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
  const selectionEnabled = options.editMode || options.filterMode
  const documents = selectionEnabled ? collectDocuments(node) : []
  const selection = options.editMode ? options.selectedUrls : options.selectedFilters
  const checkedCount = documents.filter(doc => selection?.has(doc.url)).length
  const selected = checkedCount > 0 && checkedCount === documents.length
  const mixed = options.editMode && checkedCount > 0 && !selected
  const selectionLabel = node.kind === 'folder'
    ? options.t('library.tree.selectAllIn', { title: node.title })
    : options.t(options.editMode ? 'library.tree.select' : 'library.tree.filterBy', { title: node.title })
  const toggleNodeSelection = () => {
    if (options.editMode) {
      options.onToggleSelection(documents)
    } else if (node.kind === 'folder') {
      options.onToggleAllInGroup?.(documents)
    } else {
      options.onToggleFilter?.(node.url)
    }
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
              <ChevronIcon />
            </Button>
          )}
          <div className="uploaded-library-row">
            {selectionEnabled ? (
              <span className="uploaded-library-name uploaded-library-selection">
                <input
                  className="uploaded-library-select"
                  type="checkbox"
                  checked={selected}
                  ref={input => {
                    if (input) {
                      input.indeterminate = mixed
                    }
                  }}
                  disabled={documents.length === 0 || (options.editMode && options.editActionsDisabled)}
                  aria-label={selectionLabel}
                  onClick={(event) => event.stopPropagation()}
                  onChange={(event) => {
                    event.stopPropagation()
                    toggleNodeSelection()
                  }}
                />
                {node.kind === 'folder' && <FolderIcon />}
                <span className="uploaded-library-selection-text">
                  <bdi>{node.title}</bdi>
                  {node.kind === 'document' && node.doc.textStatus === 'recognition-required' && (
                    <span className="uploaded-library-text-status">
                      {options.t('library.documents.textRecognitionRequired')}
                    </span>
                  )}
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
              <>
                <FolderIcon />
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
              </>
            ) : (
              <span className="uploaded-library-name">
                <bdi>{node.title}</bdi>
                {node.doc.textStatus === 'recognition-required' && (
                  <span className="uploaded-library-text-status">
                    {options.t('library.documents.textRecognitionRequired')}
                  </span>
                )}
              </span>
            )}
            {opening && <span className="uploaded-library-opening">{options.t('common.opening')}</span>}
            {node.kind === 'document' && !options.filterMode && (
              <>
                {options.editMode && options.onViewDocumentInfo && (
                  <button
                    className="document-row-action document-row-action-secondary uploaded-library-icon-button"
                    type="button"
                    aria-label={options.t('library.documentInfo.editLabel', { title: node.title })}
                    title={options.t('library.documentInfo.button')}
                    onClick={(event) => {
                      event.stopPropagation()
                      options.onViewDocumentInfo?.(node.doc)
                    }}
                  >
                    <EditIcon />
                  </button>
                )}
                {!options.editMode && (
                  <>
                    <button
                      className="document-row-action document-row-action-view"
                      type="button"
                      disabled={options.documentOpening}
                      onClick={(event) => {
                        event.stopPropagation()
                        if (!options.documentOpening) {
                          options.onViewDocument?.(node.url)
                        }
                      }}
                    >
                      {opening ? options.t('common.opening') : options.t('common.view')}
                    </button>
                  </>
                )}
              </>
            )}
            {options.editMode && node.kind === 'folder' && (
              <MenuTrigger>
                <Button className="document-row-action document-row-action-secondary uploaded-library-icon-button" isDisabled={options.editActionsDisabled}
                  aria-label={options.t('library.tree.folderActionsFor', { title: node.title })}>
                  <MoreActionsIcon className="uploaded-library-more-icon" />
                </Button>
                <Popover placement="bottom end" className="document-import-popover">
                  <Menu aria-label={options.t('library.tree.folderActionsFor', { title: node.title })}
                    className="document-import-options uploaded-library-folder-menu">
                    {node.depth < 4 && (
                      <MenuItem className="document-import-option" onAction={() => options.openFolderDialog(node.id, node.title)}>
                        <FolderIcon className="uploaded-library-action-icon" />{options.t('library.tree.newSubfolder')}
                      </MenuItem>
                    )}
                    <MenuItem className="document-import-option" onAction={() => options.renameFolder(node)}>
                      <EditIcon />{options.t('library.tree.rename')}
                    </MenuItem>
                    <MenuItem className="document-import-option" onAction={() => options.deleteFolder(node)}>
                      <TrashIcon className="uploaded-library-action-icon" />{options.t('library.tree.deleteFolder')}
                    </MenuItem>
                  </Menu>
                </Popover>
              </MenuTrigger>
            )}
          </div>
        </div>
      </TreeItemContent>
      {node.children.map((child) => renderNode(child, options))}
    </TreeItem>
  )
}

function FolderIcon({ className = 'uploaded-library-folder-icon' }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="M2 4h4l2 2h6v7H2Z" />
    </svg>
  )
}

function EditIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z" />
      <path d="m13.5 6.5 4 4" />
    </svg>
  )
}
