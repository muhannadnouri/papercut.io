import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { formatSavedAudiobookMetaParts } from '../utils/format'
import './SavedAudiobooksMenu.css'

interface SavedAudiobooksMenuProps {
  records: SavedAudiobookRecord[]
  selectedId: string | null
  onSelect: (record: SavedAudiobookRecord) => void
}

export function SavedAudiobooksMenu({ records, selectedId, onSelect }: SavedAudiobooksMenuProps) {
  const { t, i18n } = useTranslation()
  // Keep mixed model, voice, and runtime metadata stable across UI directions.
  const technicalT = i18n.getFixedT('en')
  const label = t('tts.controls.savedVersions', { count: records.length })
  const headingId = useId()
  const descriptionId = useId()
  const hasCurrent = records.some((record) => record.id === selectedId)

  return (
    <MenuTrigger>
      <span className="audio-saved-trigger" title={label}>
        <Button className="audio-icon-btn audio-saved-versions-btn" aria-label={label}>
          <SavedAudiobooksIcon />
          <span className="audio-saved-count" aria-hidden="true">{records.length}</span>
        </Button>
      </span>
      <Popover
        className="audio-saved-popover"
        placement="bottom end"
        offset={6}
        containerPadding={8}
        shouldFlip
      >
        <div className="audio-saved-panel">
          <div className="audio-saved-header">
            <strong id={headingId}>{label}</strong>
            {!hasCurrent && (
              <small id={descriptionId}>
                <span>{t('tts.controls.savedVersionsMismatch')}</span>
                <span>{t('tts.controls.savedVersionsAction')}</span>
              </small>
            )}
          </div>
          <Menu
            className="audio-saved-options"
            aria-labelledby={headingId}
            aria-describedby={!hasCurrent ? descriptionId : undefined}
          >
            {records.map((record) => {
              const current = record.id === selectedId
              const metaParts = formatSavedAudiobookMetaParts(
                technicalT,
                record.modelId,
                record.voice,
                record.speed,
                record.textPreprocessor,
                record.audioDurationSec,
                record.wavBytes,
              )
              const meta = metaParts.join(' • ')
              return (
                <MenuItem
                  key={record.id}
                  id={record.id}
                  className={'audio-saved-option' + (current ? ' audio-saved-option-current' : '')}
                  aria-current={current ? 'true' : undefined}
                  textValue={current ? meta + ' - ' + t('tts.controls.currentSetup') : meta}
                  onAction={() => {
                    if (!current) onSelect(record)
                  }}
                >
                  <span className="audio-saved-meta" dir="ltr">
                    <span className="audio-saved-model">{metaParts[0]}</span>
                    <span className="audio-saved-details">
                      {metaParts.slice(1).map((part) => <span key={part}>{part}</span>)}
                    </span>
                  </span>
                  {current && <small>{t('tts.controls.currentSetup')}</small>}
                </MenuItem>
              )
            })}
          </Menu>
        </div>
      </Popover>
    </MenuTrigger>
  )
}

function SavedAudiobooksIcon() {
  return (
    <svg className="audio-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 14v-2a8 8 0 0 1 16 0v2M4 14h4v7H6a2 2 0 0 1-2-2zm16 0h-4v7h2a2 2 0 0 0 2-2z" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinejoin="round" />
    </svg>
  )
}
