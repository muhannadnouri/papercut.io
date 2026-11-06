import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import { getCompletedAppReset } from './appReset'
import './AppResetDialog.css'

/**
 * One-time result of an erase. Mounted beside `App` so it shows over the fresh library
 * whichever screen loads. Failed paths appear in full, left to right even in RTL
 * locales, because the user may have to find and delete them by hand. The wipe is
 * never retried, so this notice is the only report of what was left behind.
 */
export function AppResetNotice() {
  const { t } = useTranslation()
  const [dismissed, setDismissed] = useState(false)
  const completed = getCompletedAppReset()
  if (!completed || dismissed) {
    return null
  }

  const close = () => setDismissed(true)
  const partial = completed.failedPaths.length > 0
  return (
    <AppDialog
      title={t(partial ? 'settings.erase.partialTitle' : 'settings.erase.doneTitle')}
      description={t(partial ? 'settings.erase.partialBody' : 'settings.erase.doneBody')}
      onCancel={close}
      actions={(
        <button type="button" className="app-dialog-submit" onClick={close}>
          {t('common.close')}
        </button>
      )}
    >
      {partial && (
        <ul className="app-reset-list" dir="ltr">
          {completed.failedPaths.map((path) => <li key={path}>{path}</li>)}
        </ul>
      )}
    </AppDialog>
  )
}
