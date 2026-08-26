import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import { useTranslation } from 'react-i18next'

interface ReaderBookmarkButtonProps {
  hasBookmark: boolean
  isAtBookmark: boolean
  onMove: () => void
  onRemove: () => void
  onRestore: () => void
  onSave: () => void
}

/** Save a first bookmark directly, then expose explicit actions for an existing one. */
export function ReaderBookmarkButton({
  hasBookmark,
  isAtBookmark,
  onMove,
  onRemove,
  onRestore,
  onSave,
}: ReaderBookmarkButtonProps) {
  const { t } = useTranslation()
  const buttonClass = 'reader-bookmark-btn' + (isAtBookmark ? ' reader-bookmark-btn-active' : '')

  if (!hasBookmark) {
    const label = t('reader.saveBookmark')
    return (
      <button type="button" className={buttonClass} aria-label={label} title={label} onClick={onSave}>
        <BookmarkIcon saved={false} />
      </button>
    )
  }

  const label = t('reader.bookmarkActions')
  return (
    <MenuTrigger>
      <span title={label}>
        <Button className={buttonClass} aria-label={label}>
          <BookmarkIcon saved />
        </Button>
      </span>
      <Popover
        className="reader-bookmark-popover"
        placement="left bottom"
        offset={8}
        containerPadding={8}
        shouldFlip
      >
        <Menu className="reader-bookmark-menu" aria-label={label}>
          {!isAtBookmark && (
            <MenuItem className="reader-bookmark-menu-item" onAction={onRestore}>
              <GoToBookmarkIcon />
              <span>{t('reader.returnToBookmark')}</span>
            </MenuItem>
          )}
          {!isAtBookmark && (
            <MenuItem className="reader-bookmark-menu-item" onAction={onMove}>
              <MoveBookmarkIcon />
              <span>{t('reader.moveBookmarkHere')}</span>
            </MenuItem>
          )}
          <MenuItem
            className="reader-bookmark-menu-item reader-bookmark-menu-item-danger"
            onAction={onRemove}
          >
            <RemoveBookmarkIcon />
            <span>{t('reader.removeBookmark')}</span>
          </MenuItem>
        </Menu>
      </Popover>
    </MenuTrigger>
  )
}

function BookmarkIcon({ saved }: { saved: boolean }) {
  return (
    <svg className="reader-bookmark-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M6 4.75A2.75 2.75 0 0 1 8.75 2h6.5A2.75 2.75 0 0 1 18 4.75V21l-6-3.5L6 21z" />
      {saved && <path d="m9 10.8 2 2 4-4" />}
    </svg>
  )
}

function GoToBookmarkIcon() {
  return (
    <svg className="reader-bookmark-menu-item-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="7" />
      <circle cx="12" cy="12" r="2" />
      <path d="M12 2v3m0 14v3M2 12h3m14 0h3" />
    </svg>
  )
}

function MoveBookmarkIcon() {
  return (
    <svg className="reader-bookmark-menu-item-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M6 4.75A2.75 2.75 0 0 1 8.75 2h6.5A2.75 2.75 0 0 1 18 4.75V21l-6-3.5L6 21z" />
      <path d="M12 6v7m-3-3 3 3 3-3" />
    </svg>
  )
}

function RemoveBookmarkIcon() {
  return (
    <svg className="reader-bookmark-menu-item-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" />
    </svg>
  )
}
