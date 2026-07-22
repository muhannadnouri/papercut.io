import { useRef } from 'react'
import type { TFunction } from 'i18next'
import { Popover } from 'react-aria-components'
import type { NativeAudiobookExportFormat } from '../api/nativeTts'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { SILMA_MODEL_ID } from '../types'
import './AudiobookExportMenu.css'

interface AudiobookExportMenuProps {
  t: TFunction
  record: SavedAudiobookRecord
  options: Array<{ format: NativeAudiobookExportFormat; label: string; code: string }>
  open: boolean
  disabled: boolean
  exporting: boolean
  onOpenChange: (open: boolean) => void
  onExport: (format: NativeAudiobookExportFormat) => void
}

/** Render export actions in a portal so the menu can flip around viewport edges. */
export function AudiobookExportMenu({
  t,
  record,
  options,
  open,
  disabled,
  exporting,
  onOpenChange,
  onExport,
}: AudiobookExportMenuProps) {
  const buttonRef = useRef<HTMLButtonElement | null>(null)

  return (
    <div className="audiobook-export-menu">
      <button
        ref={buttonRef}
        className="audiobook-text-action audiobook-export"
        disabled={disabled}
        aria-expanded={open}
        onClick={() => onOpenChange(!open)}
      >
        {exporting ? t('tts.audiobooks.exporting') : t('tts.audiobooks.export')}
        <span className="audiobook-export-arrow" aria-hidden="true">&#9662;</span>
      </button>
      <Popover
        className="audiobook-export-popover"
        isOpen={open}
        onOpenChange={onOpenChange}
        triggerRef={buttonRef}
        placement="bottom end"
        offset={6}
        containerPadding={8}
        shouldFlip
      >
        <div className="audiobook-export-options">
          {options.map((option) => (
            <button
              key={option.format}
              type="button"
              className="audiobook-export-option"
              onClick={() => onExport(option.format)}
            >
              <span>{option.label}</span>
              <small>
                {t('tts.audiobooks.exportAs')} · <code>{option.code}</code>
              </small>
            </button>
          ))}
          <div className="audiobook-export-note">
            <span>{t('tts.audiobooks.aiSharedNote')}</span>
            {record.modelId === SILMA_MODEL_ID ? <span>{t('tts.audiobooks.referenceVoiceNote')}</span> : null}
          </div>
        </div>
      </Popover>
    </div>
  )
}
