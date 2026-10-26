import { useTranslation } from 'react-i18next'
import type { SemanticSearchController } from './useSemanticSearch'
import './SearchActivity.css'

/** Link to live controls from both search and reader headers, without owning a job. */
export function SearchActivity({ semantic, onManage }: {
  semantic: SemanticSearchController;
  onManage: () => void
}) {
  const { t } = useTranslation()
  const phase = !semantic.busy ? semantic.activityNotice
    : semantic.provider?.downloadingFormat ? 'downloading'
      : semantic.operation === 'disable' ? 'disabling'
        : semantic.provider?.loadingFormat || semantic.operation === 'activate' ? 'loading'
          : semantic.index?.running && (semantic.cancelling || semantic.index.cancelRequested) ? 'cancelling'
            : semantic.index?.running || semantic.operation === 'build' ? 'indexing' : 'working'
  const label = phase ? t(`settings.semantic.activity.${phase}`) : ''
  const review = phase === 'failed' || phase === 'statusFailed' || phase === 'downloadFailed'
  // Coverage is completed documents, not a time estimate; never turn it into a job percentage.
  const index = phase === 'indexing' && semantic.index?.running ? semantic.index : null
  const progress = index && index.eligibleDocuments > 0 ? t('settings.semantic.documentsReady', {
    ready: index.indexedDocuments,
    total: index.eligibleDocuments,
  }) : ''
  const action = t(review ? 'settings.semantic.activity.review' : 'settings.semantic.activity.view')
  return <>
    {/* Keep a live region mounted; byte/document ticks are intentionally not announced. */}
    <span className="search-activity-announcement" role="status" aria-atomic="true">{label}</span>
    {phase && <button type="button" className="search-activity" data-review={review || undefined}
      aria-haspopup="dialog" aria-label={`${label} ${progress} ${action}`} title={action} onClick={() => {
        onManage()
        semantic.dismissActivity()
      }}>
      {semantic.busy && <span className="spinner" aria-hidden="true" />}
      <span className="search-activity-label">{label}</span>
      {progress && <span className="search-activity-count" aria-hidden="true">{index!.indexedDocuments} / {index!.eligibleDocuments}</span>}
      {review && <span>{action}</span>}
      <span className="search-activity-chevron" aria-hidden="true" />
    </button>}
  </>
}
