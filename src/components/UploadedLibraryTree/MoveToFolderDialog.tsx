import {
  Fragment,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
} from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../AppDialog/AppDialog'
import { FolderIcon } from '../icons'
import { ChevronIcon } from '../ChevronIcon/ChevronIcon'
import type { LibraryNode } from './libraryTree'

interface MoveToFolderDialogProps {
  folders: Extract<LibraryNode, { kind: 'folder' }>[]
  documents: Extract<LibraryNode, { kind: 'document' }>[]
  roots: LibraryNode[]
  busy: boolean
  disabled: boolean
  error: string
  fallbackFocusRef: RefObject<HTMLButtonElement | null>
  onCancel: () => void
  onMove: (documentIds: string[], folderIds: string[], destination: string | null) => void
}

export function MoveToFolderDialog({ documents, folders, roots, busy, disabled, error, fallbackFocusRef, onCancel, onMove }: MoveToFolderDialogProps) {
  const { t } = useTranslation()
  const [path, setPath] = useState<string[]>([])
  const [confirming, setConfirming] = useState(false)
  const cancelRef = useRef<HTMLButtonElement>(null)
  const pathRef = useRef<HTMLParagraphElement>(null)
  const visited: LibraryNode[] = []
  let children = roots
  for (const id of path) {
    const folder = children.find(node => node.kind === 'folder' && node.id === id)
    if (!folder) {
      break
    }
    visited.push(folder)
    children = folder.children
  }

  // Confirmation reuses the scrolled picker body. Show the destination before the first paint.
  useLayoutEffect(() => {
    const body = pathRef.current?.parentElement
    if (confirming && body) {
      body.scrollTop = 0
    }
  }, [confirming])

  // The folder button that opened this level leaves the DOM. Keep keyboard focus at the new path.
  useEffect(() => {
    if (confirming) {
      cancelRef.current?.focus({ preventScroll: true })
    } else {
      pathRef.current?.focus()
    }
  }, [path, confirming])

  // Only direct children are already here. Documents deeper in this branch can still move up.
  const residentIds = new Set(children.filter(node => node.kind === 'document').map(node => node.id))
  const documentsToMove = documents.filter(doc => !residentIds.has(doc.id))
  const residentFolderIds = new Set(children.filter(node => node.kind === 'folder').map(node => node.id))
  const foldersToMove = folders.filter(folder => !residentFolderIds.has(folder.id))
  const crumbs = [t('library.tree.root'), ...visited.map(folder => folder.title)]
  const cancel = () => {
    if (confirming) {
      setConfirming(false)
    } else {
      onCancel()
    }
  }

  return <AppDialog title={t(confirming ? foldersToMove.length ? 'library.tree.confirmMoveItems' : 'library.tree.confirmMove' : 'library.tree.moveToFolder')} onCancel={cancel}
    navigationDisabled={busy} fallbackFocusRef={fallbackFocusRef} showCloseButton
    onBack={!confirming && path.length ? () => setPath(path.slice(0, -1)) : undefined}
    actions={<>
      {confirming && <button type="button" className="app-dialog-cancel" ref={cancelRef}
        disabled={busy} onClick={cancel}>{t('common.cancel')}</button>}
      <button className="app-dialog-submit" type="button"
        disabled={disabled || busy || (!documentsToMove.length && !foldersToMove.length) || visited.length !== path.length}
        onClick={() => {
          if (confirming) {
            onMove(documentsToMove.map(doc => doc.id), foldersToMove.map(folder => folder.id), visited.at(-1)?.id ?? null)
          } else {
            setConfirming(true)
          }
        }}>{t(confirming ? 'library.tree.move' : 'library.tree.moveHere')}</button>
    </>}>
    <p className="uploaded-library-move-path" ref={pathRef} tabIndex={-1}>
      {crumbs.map((title, index) => <Fragment key={index}>
        {index > 0 && <span aria-hidden="true"> / </span>}
        {!confirming && index < visited.length
          ? <button type="button" onClick={() => setPath(path.slice(0, index))}><bdi>{title}</bdi></button>
          : <bdi aria-current={index === visited.length ? 'location' : undefined}>{title}</bdi>}
      </Fragment>)}
    </p>
    {confirming ? <>
      {foldersToMove.length > 0 && <>
        <strong>{t('library.tree.folders')} ({foldersToMove.length.toLocaleString()})</strong>
        <ul className="uploaded-library-move-documents">
          {foldersToMove.map(folder => <li key={folder.id}><bdi>{folder.title}</bdi></li>)}
        </ul>
        <p>{t('library.tree.moveFolderContents')}</p>
      </>}
      {documentsToMove.length > 0 && <><strong>{t('library.tree.selectedCount', { count: documentsToMove.length })}</strong>
        <ul className="uploaded-library-move-documents">
          {documentsToMove.map(doc => <li key={doc.id}><bdi>{doc.title}</bdi></li>)}
        </ul></>}
    </> : <div className="uploaded-library-destinations">
      {children.filter(node => node.kind === 'folder').map(folder => (
        <button key={folder.id} type="button" className="uploaded-library-destination"
          disabled={busy || folders.some(source => source.id === folder.id)} onClick={() => setPath([...path, folder.id])}>
          <FolderIcon className="uploaded-library-folder-icon" /><bdi>{folder.title}</bdi><ChevronIcon />
        </button>
      ))}
    </div>}
    {error && <p role="alert" className="app-dialog-error">{error}</p>}
  </AppDialog>
}
