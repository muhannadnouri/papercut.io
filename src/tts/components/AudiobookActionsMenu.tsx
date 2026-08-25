import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import type { TFunction } from 'i18next'
import type { NativeAudiobookExportFormat } from '../api/nativeTts'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { SILMA_MODEL_ID } from '../types'
import './AudiobookActionsMenu.css'

interface AudiobookActionsMenuProps {
  t: TFunction
  record: SavedAudiobookRecord
  options: Array<{ format: NativeAudiobookExportFormat; label: string; code: string }>
  open: boolean
  disabled: boolean
  onOpenChange: (open: boolean) => void
  onExport: (format: NativeAudiobookExportFormat) => void
  onDelete: () => void
}

/** Render saved-audiobook actions in a portal so the menu can flip around viewport edges. */
export function AudiobookActionsMenu({
  t,
  record,
  options,
  open,
  disabled,
  onOpenChange,
  onExport,
  onDelete,
}: AudiobookActionsMenuProps) {
  const label = t('tts.audiobooks.moreActions')
  return (
    <div className="audiobook-actions-menu">
      <MenuTrigger isOpen={open} onOpenChange={onOpenChange}>
        <Button
          className="audiobook-actions-trigger"
          isDisabled={disabled}
          aria-label={label}
        >
          <AudiobookMoreActionsIcon />
        </Button>
        <Popover
          className="audiobook-actions-popover"
          placement="bottom end"
          offset={6}
          containerPadding={8}
          shouldFlip
        >
          <div className="audiobook-actions-surface">
            <Menu className="audiobook-actions-options" aria-label={label}>
              {options.map((option) => (
                <MenuItem
                  key={option.format}
                  id={option.format}
                  className="audiobook-actions-option"
                  textValue={option.label}
                  onAction={() => onExport(option.format)}
                >
                  <span>{option.label}</span>
                  <small>
                    {t('tts.audiobooks.exportAs')} · <code>{option.code}</code>
                  </small>
                </MenuItem>
              ))}
              <MenuItem
                id="delete"
                className="audiobook-actions-option audiobook-actions-option-danger"
                textValue={t('tts.audiobooks.delete')}
                onAction={onDelete}
              >
                <AudiobookDeleteIcon />
                <span>{t('tts.audiobooks.delete')}</span>
              </MenuItem>
            </Menu>
            <div className="audiobook-actions-note">
              <span>{t('tts.audiobooks.aiSharedNote')}</span>
              {record.modelId === SILMA_MODEL_ID ? <span>{t('tts.audiobooks.referenceVoiceNote')}</span> : null}
            </div>
          </div>
        </Popover>
      </MenuTrigger>
    </div>
  )
}

function AudiobookMoreActionsIcon() {
  return (
    <svg className="audiobook-actions-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="5" r="2" />
      <circle cx="12" cy="12" r="2" />
      <circle cx="12" cy="19" r="2" />
    </svg>
  )
}

function AudiobookDeleteIcon() {
  return (
    <svg className="audiobook-actions-option-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 7h16M9 7V4h6v3m3 0-1 14H7L6 7m4 4v6m4-6v6" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}
