import {
  useEffect,
  useState,
  type RefObject,
} from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import { currentAppLocale } from '../i18n'
import { formatStorageSize } from '../utils/formatUtils'
import {
  eraseAppData,
  getAppResetSummary,
  type AppResetBlocker,
  type AppResetSummary,
} from './appReset'
import './AppResetDialog.css'

interface AppResetDialogProps {
  /** A job only the frontend knows about, such as OCR running in the WebView. */
  blockedBy: AppResetBlocker | null
  fallbackFocusRef: RefObject<HTMLElement | null>
  onBack: () => void
  onClose: () => void
  onExportFirst: () => void
}

/**
 * Confirmation view for erasing all app data, shown in place of root Settings so
 * there is never a nested focus trap.
 *
 * Follows NN/g guidance for an irreversible action: specific consequences with live
 * counts, outcome-named buttons, and a separate acknowledgement. Bookmarks and settings
 * have no backup, so the export note must stay honest about what export omits.
 *
 * A job can be reported from three places, in priority order: `blockedBy` for
 * WebView-only work such as OCR, a refusal from the request itself, and the summary
 * fetched on open. A refusal is never cleared here because the job may still be
 * running; going Back and reopening fetches a fresh summary.
 */
export function AppResetDialog({
  blockedBy,
  fallbackFocusRef,
  onBack,
  onClose,
  onExportFirst,
}: AppResetDialogProps) {
  const { t } = useTranslation()
  const [summary, setSummary] = useState<AppResetSummary | null>(null)
  const [refusedBy, setRefusedBy] = useState<AppResetBlocker | null>(null)
  const [acknowledged, setAcknowledged] = useState(false)
  const [erasing, setErasing] = useState(false)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let current = true
    getAppResetSummary()
      .then((next) => {
        if (current) {
          setSummary(next)
        }
      })
      .catch(() => {
        if (current) {
          setFailed(true)
        }
      })
    return () => {
      current = false
    }
  }, [])

  const blocker = blockedBy ?? refusedBy ?? summary?.blockedBy ?? null
  const totalSize = summary && formatStorageSize(summary.totalBytes)
  const canErase = summary !== null && blocker === null && acknowledged && !erasing

  const erase = async () => {
    setErasing(true)
    setFailed(false)
    try {
      const refused = await eraseAppData()
      if (refused) {
        setRefusedBy(refused)
        setErasing(false)
      }
      // Otherwise stay busy until the restart or reload replaces this page.
    } catch {
      setFailed(true)
      setErasing(false)
    }
  }

  return (
    <AppDialog
      className="app-reset-dialog"
      title={t('settings.erase.title')}
      description={t('settings.erase.description')}
      showCloseButton
      navigationDisabled={erasing}
      fallbackFocusRef={fallbackFocusRef}
      onBack={onBack}
      onCancel={onClose}
      actions={(
        <>
          {/* In the pinned footer, so a disabled button never appears without its reason. */}
          <label className="app-reset-acknowledge">
            <input
              type="checkbox"
              checked={acknowledged}
              disabled={erasing}
              onChange={(event) => setAcknowledged(event.target.checked)}
            />
            <span>{t('settings.erase.acknowledge')}</span>
          </label>
          {blocker && (
            <p className="app-reset-blocked" role="status">
              {t(`settings.erase.blocked.${blocker}`)}
            </p>
          )}
          {failed && (
            <p className="app-dialog-error app-reset-error" role="alert">{t('settings.erase.failed')}</p>
          )}
          <button type="button" className="app-dialog-cancel" disabled={erasing} onClick={onBack}>
            {t('settings.erase.keep')}
          </button>
          <button
            type="button"
            className="app-dialog-submit app-dialog-submit-danger"
            disabled={!canErase}
            onClick={() => {
              void erase()
            }}
          >
            {erasing ? t('settings.erase.erasing') : t('settings.erase.confirm')}
          </button>
        </>
      )}
    >
      <h3 className="app-reset-heading">{t('settings.erase.removedHeading')}</h3>
      <ul className="app-reset-list">
        <li>{t('settings.erase.removedDocuments')}</li>
        <li>{t('settings.erase.removedAudiobooks')}</li>
        <li>{t('settings.erase.removedPreferences')}</li>
        <li>{t('settings.erase.removedModels')}</li>
      </ul>
      {summary && (
        <dl className="app-dialog-details">
          <div className="app-dialog-detail-row">
            <dt>{t('settings.erase.documentCount')}</dt>
            <dd>{summary.documents.toLocaleString(currentAppLocale())}</dd>
          </div>
          <div className="app-dialog-detail-row">
            <dt>{t('settings.erase.audiobookCount')}</dt>
            <dd>{summary.audiobooks.toLocaleString(currentAppLocale())}</dd>
          </div>
          {totalSize && (
            <div className="app-dialog-detail-row">
              <dt>{t('settings.erase.totalSize')}</dt>
              <dd dir="auto">{totalSize}</dd>
            </div>
          )}
        </dl>
      )}

      {summary && summary.locations.length > 0 && (
        <>
          <h3 className="app-reset-heading">{t('settings.erase.locationHeading')}</h3>
          {/* Paths read left to right in every locale, and users may need to copy them. */}
          <ul className="app-reset-list app-reset-locations" dir="ltr">
            {summary.locations.map((location) => <li key={location}>{location}</li>)}
          </ul>
          <p className="app-reset-note">{t('settings.erase.locationNote')}</p>
        </>
      )}

      <h3 className="app-reset-heading">{t('settings.erase.keptHeading')}</h3>
      <ul className="app-reset-list">
        <li>{t('settings.erase.keptExports')}</li>
        <li>{t('settings.erase.keptOriginals')}</li>
        <li>{t('settings.erase.keptStarter')}</li>
        <li>{t('settings.erase.keptBackups')}</li>
      </ul>

      <div className="app-reset-export">
        <button type="button" className="app-settings-data-action" disabled={erasing} onClick={onExportFirst}>
          {t('settings.erase.exportFirst')}
        </button>
        <p>{t('settings.erase.exportNote')}</p>
      </div>
    </AppDialog>
  )
}
