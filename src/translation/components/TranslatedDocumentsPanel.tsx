import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../../components/AppDialog/AppDialog'
import { Panel } from '../../components/Panel/Panel'
import type { TranslatedDocumentInfo, TranslationDeleteResult } from '../api/nativeTranslation'
import { formatLanguageLabel, formatStatusLabel } from '../utils/presentation'

interface TranslatedDocumentsPanelProps {
  translatedDocuments: TranslatedDocumentInfo[]
  modelNameById: ReadonlyMap<string, string>
  deleteState: TranslationDeleteResult | null
  onDeleteTranslatedDocument: (id: string) => Promise<void>
  onOpenTranslatedDocument: (url: string) => void | Promise<void>
}

/** Own saved-list presentation only; native deletion and Library refresh stay in the manager. */
export function TranslatedDocumentsPanel({
  translatedDocuments,
  modelNameById,
  deleteState,
  onDeleteTranslatedDocument,
  onOpenTranslatedDocument,
}: TranslatedDocumentsPanelProps) {
  const { t, i18n } = useTranslation()
  const locale = i18n.resolvedLanguage ?? i18n.language
  const [docsOpen, setDocsOpen] = useState(true)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState('')
  // Refresh can remove or rename a row while its dialog is open; never retain a stale document snapshot.
  const confirmingDocument = useMemo(
    () => translatedDocuments.find((doc) => doc.id === confirmingDeleteId) ?? null,
    [confirmingDeleteId, translatedDocuments],
  )
  // Dismissal is local to this list; it must not hide job or model-operation outcomes.
  const [dismissedStatusKeys, setDismissedStatusKeys] = useState<string[]>([])
  const deleteStatusKey = deleteState ? 'delete:' + deleteState.id + ':' + deleteState.message : ''
  const showDeleteStatus = Boolean(deleteState) && !dismissedStatusKeys.includes(deleteStatusKey)
  const dismissStatus = (key: string) => {
    setDismissedStatusKeys(previous => previous.includes(key) ? previous : [...previous, key])
  }

  return (
    <>
      <Panel
        className="translation-subpanel"
        ariaLabel={t('translation.documents.ariaLabel')}
        title={t('translation.documents.title')}
        meta={t('translation.documents.savedCount', { count: translatedDocuments.length })}
        open={docsOpen}
        onToggle={() => setDocsOpen((value) => !value)}
      >
        {deleteState && showDeleteStatus && (
          <div className={'translation-alert' + (deleteState.deleted ? '' : ' translation-alert-error')} role="status">
            <button
              type="button"
              className="translation-alert-dismiss"
              aria-label={t('translation.status.dismissDelete')}
              onClick={() => dismissStatus(deleteStatusKey)}
            >
              ×
            </button>
            <span className="translation-alert-message">
              {deleteState.deleted
                ? t('translation.status.documentDeleted')
                : deleteState.message}
            </span>
          </div>
        )}
        {translatedDocuments.length > 0 ? (
          <div className="translation-document-list">
            {translatedDocuments.map((doc) => (
              <div key={doc.id} className="translation-document-item">
                <div>
                  <strong>{doc.title}</strong>
                  <span>
                    {formatLanguageLabel(doc.sourceLanguage, locale, t)}
                    {' → '}
                    {formatLanguageLabel(doc.targetLanguage, locale, t)}
                    {' · '}{modelNameById.get(doc.modelId) ?? doc.modelId}
                    {' · '}{formatStatusLabel(doc.status, t)}
                  </span>
                </div>
                <div className="translation-document-actions">
                  <button
                    type="button"
                    className="translation-document-view-btn"
                    aria-label={t('translation.documents.viewLabel', { title: doc.title })}
                    onClick={() => { void onOpenTranslatedDocument(doc.documentUrl) }}
                  >
                    {t('common.view')}
                  </button>
                  <button
                    type="button"
                    className="translation-document-delete-btn"
                    aria-label={t('translation.documents.deleteLabel', { title: doc.title })}
                    onClick={() => setConfirmingDeleteId(doc.id)}
                  >
                    {t('common.delete')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="translation-section-empty">{t('translation.documents.empty')}</p>
        )}
      </Panel>
      {confirmingDocument && (
        <AppDialog
          title={t('translation.confirmDelete.title')}
          description={t('translation.confirmDelete.description', { title: confirmingDocument.title })}
          onCancel={() => setConfirmingDeleteId('')}
          actions={
            <>
              <button
                type="button"
                className="app-dialog-cancel"
                onClick={() => setConfirmingDeleteId('')}
              >
                {t('translation.confirmDelete.keep')}
              </button>
              <button
                type="button"
                className="app-dialog-danger"
                onClick={() => {
                  setConfirmingDeleteId('')
                  void onDeleteTranslatedDocument(confirmingDocument.id)
                }}
              >
                {t('common.delete')}
              </button>
            </>
          }
        />
      )}
    </>
  )
}
