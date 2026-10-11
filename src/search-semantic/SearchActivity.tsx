import { useTranslation } from 'react-i18next'
import type { SemanticSearchController } from './useSemanticSearch'

/** Link to live controls from both search and reader headers, without owning a job. */
export function SearchActivity({ semantic, onManage }: {
  semantic: SemanticSearchController; onManage: () => void
}) {
  const { t } = useTranslation()
  if (!semantic.busy) return null
  const phase = semantic.provider?.downloadingFormat ? 'downloading'
    : semantic.operation === 'disable' ? 'disabling'
    : semantic.provider?.loadingFormat || semantic.operation === 'activate' ? 'loading'
    : semantic.index?.running || semantic.operation === 'build' ? 'indexing' : 'working'
  return <button type="button" className="search-activity" aria-haspopup="dialog" onClick={onManage}>
    <span role="status">{t(`settings.semantic.${phase}`)}</span>
    <span>{t('settings.semantic.manage')}</span>
  </button>
}
