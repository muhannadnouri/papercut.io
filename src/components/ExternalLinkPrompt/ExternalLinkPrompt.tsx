import { useTranslation } from 'react-i18next'
import { AppDialog } from '../AppDialog/AppDialog'

interface ExternalLinkPromptProps {
  url: string
  error?: string
  onCancel: () => void
  onOpen: () => void | Promise<void>
}

export function ExternalLinkPrompt({ url, error, onCancel, onOpen }: ExternalLinkPromptProps) {
  const { t } = useTranslation()

  return (
    <AppDialog
      title={t('externalLink.title')}
      description={t('externalLink.description')}
      onCancel={onCancel}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" onClick={onCancel}>{t('common.cancel')}</button>
          <button type="button" className="app-dialog-submit" onClick={onOpen}>{t('common.open')}</button>
        </>
      )}
    >
      <code className="app-dialog-code" dir="ltr">{url}</code>
      {error && <p className="app-dialog-error" role="alert">{error}</p>}
    </AppDialog>
  )
}
