import { MoveToFolderDialog } from './MoveToFolderDialog'
import {
  FolderIcon,
  MoreActionsIcon,
  MoveToFolderIcon,
} from '../icons'
import {
  useEffect,
  useMemo,
  useRef,
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
  onMoveFolder?: (folderId: string, parentId: string | null) => Promise<void> | void
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
  onMoveFolder,
  onRenameFolder,
  onToggleAllInGroup,
  onToggleFilter,
  onViewAudiobooks,
  onViewDocumentInfo,
  onViewDocument,
}: UploadedLibraryTreeProps) {
  const { t, i18n } = useTranslation()
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set())
  const [expandedKeys, setExpandedKeys] = useState<Set<Key>>(new Set())
  const [rootCollapsed, setRootCollapsed] = useState(false)
  const doneRef = useRef<HTMLButtonElement>(null)
  const [moving, setMoving] = useState(false)
  const [folderDialog, setFolderDialog] = useState<FolderDialogState | null>(null)
  const [folderDialogError, setFolderDialogError] = useState('')
  const [actionError, setActionError] = useState('')
  const [busy, setBusy] = useState(false)
  const { confirm: confirmLibraryAction, dialog: libraryConfirmationDialog } = useAppConfirmation()
  const filterMode = mode === 'filter'
  const editMode = Boolean(onDone)
  const organizing = mode === 'library' && editMode
  const locale = i18n.resolvedLanguage ?? i18n.language
  const { nodes, folders, nodeByKey } = useMemo(
    () => buildLibraryTree(documents, organization, {
      hideEmptyFolders: filterMode || filterActive,
      locale,
    }),
    [documents, filterActive, filterMode, locale, organization],
  )
  // Destructive counts must include documents hidden by active Library filters.
  const unfilteredTree = useMemo(
    () => buildLibraryTree(unfilteredDocuments, organization, { locale }),
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
  const selectedDocuments = documentNodes.filter(node => selectedKeys.has(node.key))
  const selectedFolders = Array.from(nodeByKey.values()).filter((node): node is FolderNode =>
    node.kind === 'folder' && selectedKeys.has(node.key))
  const coveredKeys = new Set<string>()
  for (const folder of selectedFolders) {
    visitDescendants(unfilteredTree.nodeByKey.get(folder.key) ?? folder, child => coveredKeys.add(child.key))
  }
  const hasSelection = selectedDocuments.length + selectedFolders.length > 0
  const selectedDocumentIds = selectedDocuments.map(node => node.id)
  const canMoveDocuments = organizing && !mutationDisabled && hasSelection && (!selectedFolders.length || Boolean(onMoveFolder))
  const canDeleteDocuments = !selectedFolders.length && canMoveDocuments && !busy && Boolean(onDeleteDocuments)
  const allDocumentsSelected = documentNodes.length > 0 && selectedDocuments.length === documentNodes.length

  useEffect(() => {
    setActionError('')
  }, [selectedKeys, editMode])

  useEffect(() => {
    if (editMode) {
      setRootCollapsed(false)
    } else {
      setSelectedKeys(new Set())
      setMoving(false)
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

  // Selecting a folder replaces explicit selections below it, including ones hidden by a filter.
  const toggleSelection = (node: LibraryNode) => {
    if (busy || mutationDisabled || coveredKeys.has(node.key)) {
      return
    }
    setSelectedKeys(current => {
      const next = new Set(current)
      if (!next.delete(node.key)) {
        next.add(node.key)
        visitDescendants(unfilteredTree.nodeByKey.get(node.key) ?? node, child => next.delete(child.key))
      }
      return next
    })
  }

  const selectAllDocuments = () => {
    setSelectedKeys(new Set(documentNodes.map(node => node.key)))
  }

  const clearSelection = () => {
    setSelectedKeys(new Set())
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
      toggleSelection(node)
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
    const fullFolder = unfilteredTree.nodeByKey.get(folder.key)
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

  const moveSelectedItems = (documentIds: string[], folderIds: string[], destination: string | null) => {
    if (!canMoveDocuments) {
      return
    }
    if (!onMoveDocuments) {
      return
    }
    setActionError('')
    void (async () => {
      try {
        await runEditAction(async () => {
          for (const id of folderIds) {
            if (!onMoveFolder) {
              throw new Error('Folder move unavailable')
            }
            await onMoveFolder(id, destination)
          }
          if (documentIds.length) {
            await onMoveDocuments(documentIds, destination)
          }
        })
        setMoving(false)
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
                ref={doneRef}
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
                  {selectedFolders.length > 0 && <>{t('library.tree.folders')} ({selectedFolders.length.toLocaleString(locale)})</>}
                  {selectedFolders.length > 0 && selectedDocuments.length > 0 && ' · '}
                  {(!selectedFolders.length || selectedDocuments.length > 0) && t('library.tree.selectedCount', { count: selectedDocumentIds.length })}
                </strong>
                <button
                  type="button"
                  disabled={busy || mutationDisabled || documentNodes.length === 0}
                  onClick={allDocumentsSelected ? clearSelection : selectAllDocuments}
                >
                  {allDocumentsSelected ? t('common.deselectAll') : t('common.selectAll')}
                </button>
                {hasSelection && !allDocumentsSelected && (
                  <button type="button" disabled={busy || mutationDisabled} onClick={clearSelection}>
                    {t('common.deselectAll')}
                  </button>
                )}
              </div>
              {hasSelection && (
                <div className="uploaded-library-task-actions">
                  <button type="button" className="uploaded-library-move-trigger uploaded-library-icon-action" disabled={busy || !canMoveDocuments}
                    onClick={() => {
                      setActionError('')
                      setMoving(true)
                    }}>
                    <MoveToFolderIcon className="uploaded-library-action-icon" />
                    {t('library.tree.move')}
                  </button>
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
            {actionError && !moving && (
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
            selectedKeys,
            coveredKeys,
            editActionsDisabled: busy || mutationDisabled,
            renameFolder,
            deleteFolder,
            locale,
            t,
            openFolderDialog,
          }))}
        </Tree>
      )}
      {moving && organizing && (
        <MoveToFolderDialog documents={selectedDocuments} folders={selectedFolders} roots={unfilteredTree.nodes} busy={busy} disabled={!canMoveDocuments}
          error={actionError} fallbackFocusRef={doneRef}
          onCancel={() => {
            setMoving(false)
            setActionError('')
          }} onMove={moveSelectedItems} />
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

function visitDescendants(node: LibraryNode, visit: (child: LibraryNode) => void) {
  for (const child of node.children) {
    visit(child)
    visitDescendants(child, visit)
  }
}

function hasSelectedDescendant(node: LibraryNode, keys: Set<string>): boolean {
  return node.children.some(child => keys.has(child.key) || hasSelectedDescendant(child, keys))
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
  onToggleSelection: (node: LibraryNode) => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onViewDocument?: (url: string) => void
  selectedFilters?: Set<string>
  coveredKeys: Set<string>
  selectedKeys: Set<string>
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
  const documents = options.filterMode ? collectDocuments(node) : []
  const selected = options.editMode
    ? options.selectedKeys.has(node.key) || options.coveredKeys.has(node.key)
    : documents.length > 0 && documents.every(doc => options.selectedFilters?.has(doc.url))
  const mixed = options.editMode && !selected && hasSelectedDescendant(node, options.selectedKeys)
  const selectionLabel = options.t(options.editMode ? 'library.tree.select'
    : node.kind === 'folder' ? 'library.tree.selectAllIn' : 'library.tree.filterBy', { title: node.title })
  const toggleNodeSelection = () => {
    if (options.editMode) {
      options.onToggleSelection(node)
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
                  disabled={options.editMode ? options.coveredKeys.has(node.key) || options.editActionsDisabled : !documents.length}
                  aria-label={selectionLabel}
                  onClick={(event) => event.stopPropagation()}
                  onChange={(event) => {
                    event.stopPropagation()
                    toggleNodeSelection()
                  }}
                />
                {node.kind === 'folder' && <FolderIcon className="uploaded-library-folder-icon" />}
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
                <FolderIcon className="uploaded-library-folder-icon" />
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
                      <MenuItem className="document-import-option" textValue={options.t('library.tree.newSubfolder')}
                        onAction={() => options.openFolderDialog(node.id, node.title)}>
                        <FolderIcon className="uploaded-library-action-icon" />{options.t('library.tree.newSubfolder')}
                      </MenuItem>
                    )}
                    <MenuItem className="document-import-option" textValue={options.t('library.tree.rename')}
                      onAction={() => options.renameFolder(node)}>
                      <EditIcon />{options.t('library.tree.rename')}
                    </MenuItem>
                    <MenuItem className="document-import-option" textValue={options.t('library.tree.deleteFolder')}
                      onAction={() => options.deleteFolder(node)}>
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

function EditIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z" />
      <path d="m13.5 6.5 4 4" />
    </svg>
  )
}
