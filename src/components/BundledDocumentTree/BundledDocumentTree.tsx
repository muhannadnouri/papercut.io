import { useMemo, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, Tree, TreeItem, TreeItemContent, type Key } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import {
  buildBundledDocumentTree,
  type BundledDocumentFolder,
} from '../DocumentBrowser/bundledDocuments'
import '../UploadedLibraryTree/UploadedLibraryTree.css'

interface BundledDocumentTreeProps {
  documents: DocumentInfo[]
  mode?: 'library' | 'filter'
  filterActive?: boolean
  documentOpening?: boolean
  openingDocumentUrl?: string
  selectedFilters?: Set<string>
  onToggleAllInGroup?: (docs: DocumentInfo[]) => void
  onToggleFilter?: (url: string) => void
  onViewDocument?: (url: string) => void
}

type IndexedItem =
  | { kind: 'folder'; folder: BundledDocumentFolder }
  | { kind: 'document'; document: DocumentInfo }

/** Render the filesystem hierarchy shipped under `public/documents` without
 * giving static bundled folders uploaded-library editing behavior. */
export function BundledDocumentTree({
  documents,
  mode = 'library',
  filterActive = false,
  documentOpening = false,
  openingDocumentUrl,
  selectedFilters,
  onToggleAllInGroup,
  onToggleFilter,
  onViewDocument,
}: BundledDocumentTreeProps) {
  const { t, i18n } = useTranslation()
  const locale = i18n.resolvedLanguage ?? i18n.language
  const tree = useMemo(
    () => buildBundledDocumentTree(documents, locale),
    [documents, locale],
  )
  const { allFolderKeys, itemByKey, topFolderKeys } = useMemo(
    () => indexTree(tree.folders, tree.documents),
    [tree],
  )
  const [expandedKeys, setExpandedKeys] = useState<Set<Key>>(
    () => new Set(topFolderKeys),
  )
  const filterMode = mode === 'filter'
  const visibleExpandedKeys = filterActive ? new Set(allFolderKeys) : expandedKeys

  const toggleFolderExpanded = (key: string) => {
    setExpandedKeys((current) => {
      const next = new Set(current)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  const handleAction = (key: Key) => {
    const item = itemByKey.get(String(key))
    if (!item) return
    if (item.kind === 'folder') {
      if (filterMode) onToggleAllInGroup?.(collectDocuments(item.folder))
      else toggleFolderExpanded(String(key))
      return
    }
    if (documentOpening) return
    if (filterMode) onToggleFilter?.(item.document.url)
    else onViewDocument?.(item.document.url)
  }

  const renderDocument = (document: DocumentInfo): ReactNode => {
    const key = documentKey(document)
    const selected = selectedFilters?.has(document.url) ?? false
    const opening = openingDocumentUrl === document.url
    return (
      <TreeItem
        key={key}
        id={key}
        textValue={document.title}
        className={[
          'uploaded-library-item uploaded-library-document',
          'uploaded-library-item-actionable',
          selected ? 'uploaded-library-item-selected' : '',
          opening ? 'uploaded-library-item-opening' : '',
        ].filter(Boolean).join(' ')}
      >
        <TreeItemContent>
          <div className="uploaded-library-content">
            <div className="uploaded-library-row">
              {filterMode ? (
                <span className="uploaded-library-name uploaded-library-selection">
                  <input
                    className="uploaded-library-select"
                    type="checkbox"
                    checked={selected}
                    aria-label={t('library.tree.filterBy', { title: document.title })}
                    onClick={(event) => event.stopPropagation()}
                    onChange={(event) => {
                      event.stopPropagation()
                      onToggleFilter?.(document.url)
                    }}
                  />
                  <bdi className="uploaded-library-selection-text">{document.title}</bdi>
                </span>
              ) : (
                <bdi className="uploaded-library-name">{document.title}</bdi>
              )}
              {opening && <span className="uploaded-library-opening">{t('common.opening')}</span>}
              {!filterMode && (
                <button
                  className="document-row-action document-row-action-view"
                  type="button"
                  disabled={documentOpening}
                  onClick={(event) => {
                    event.stopPropagation()
                    if (!documentOpening) onViewDocument?.(document.url)
                  }}
                >
                  {opening ? t('common.opening') : t('common.view')}
                </button>
              )}
            </div>
          </div>
        </TreeItemContent>
      </TreeItem>
    )
  }

  const renderFolder = (folder: BundledDocumentFolder): ReactNode => {
    const key = folderKey(folder)
    const folderDocuments = collectDocuments(folder)
    const selected = folderDocuments.length > 0 &&
      folderDocuments.every((document) => selectedFilters?.has(document.url))
    const expanded = visibleExpandedKeys.has(key)
    return (
      <TreeItem
        key={key}
        id={key}
        textValue={folder.name}
        className={[
          'uploaded-library-item uploaded-library-folder',
          'uploaded-library-item-actionable',
          selected ? 'uploaded-library-item-selected' : '',
        ].filter(Boolean).join(' ')}
      >
        <TreeItemContent>
          <div className="uploaded-library-content">
            <Button slot="chevron" className="uploaded-library-chevron">
              &#9656;
            </Button>
            <div className="uploaded-library-row">
              {filterMode ? (
                <span className="uploaded-library-name uploaded-library-selection">
                  <input
                    className="uploaded-library-select"
                    type="checkbox"
                    checked={selected}
                    aria-label={t('library.tree.selectAllIn', { title: folder.name })}
                    onClick={(event) => event.stopPropagation()}
                    onChange={(event) => {
                      event.stopPropagation()
                      onToggleAllInGroup?.(folderDocuments)
                    }}
                  />
                  <span className="uploaded-library-selection-text">
                    <bdi>{folder.name}</bdi>{' '}
                    <span className="uploaded-library-folder-count">
                      ({folder.documentCount.toLocaleString(locale)})
                    </span>
                  </span>
                </span>
              ) : (
                <button
                  className="uploaded-library-name uploaded-library-name-button"
                  type="button"
                  aria-expanded={expanded}
                  onClick={(event) => {
                    event.stopPropagation()
                    toggleFolderExpanded(key)
                  }}
                >
                  <bdi>{folder.name}</bdi>{' '}
                  <span className="uploaded-library-folder-count">
                    ({folder.documentCount.toLocaleString(locale)})
                  </span>
                </button>
              )}
            </div>
          </div>
        </TreeItemContent>
        {folder.folders.map(renderFolder)}
        {folder.documents.map(renderDocument)}
      </TreeItem>
    )
  }

  if (tree.documentCount === 0) return null

  return (
    <Tree
      aria-label={t('library.documents.ariaLabel')}
      className="uploaded-library-tree"
      keyboardNavigationBehavior="tab"
      selectionMode="none"
      expandedKeys={visibleExpandedKeys}
      onExpandedChange={setExpandedKeys}
      onAction={handleAction}
      disabledKeys={documentOpening
        ? Array.from(itemByKey)
          .filter(([, item]) => item.kind === 'document')
          .map(([key]) => key)
        : undefined}
    >
      {tree.folders.map(renderFolder)}
      {tree.documents.map(renderDocument)}
    </Tree>
  )
}

function indexTree(
  folders: BundledDocumentFolder[],
  documents: DocumentInfo[],
): {
  allFolderKeys: string[]
  itemByKey: Map<string, IndexedItem>
  topFolderKeys: string[]
} {
  const allFolderKeys: string[] = []
  const itemByKey = new Map<string, IndexedItem>()
  const visitFolder = (folder: BundledDocumentFolder) => {
    const key = folderKey(folder)
    allFolderKeys.push(key)
    itemByKey.set(key, { kind: 'folder', folder })
    folder.folders.forEach(visitFolder)
    folder.documents.forEach((document) => {
      itemByKey.set(documentKey(document), { kind: 'document', document })
    })
  }
  folders.forEach(visitFolder)
  documents.forEach((document) => {
    itemByKey.set(documentKey(document), { kind: 'document', document })
  })
  return {
    allFolderKeys,
    itemByKey,
    topFolderKeys: folders.map(folderKey),
  }
}

function collectDocuments(folder: BundledDocumentFolder): DocumentInfo[] {
  return [
    ...folder.documents,
    ...folder.folders.flatMap(collectDocuments),
  ]
}

function folderKey(folder: BundledDocumentFolder): string {
  return 'bundled-folder:' + folder.id
}

function documentKey(document: DocumentInfo): string {
  return 'bundled-document:' + document.url
}
