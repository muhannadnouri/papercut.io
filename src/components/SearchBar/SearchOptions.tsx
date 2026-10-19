import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { SearchMode } from '../../types/search'
import './SearchBar.css'

export function SearchOptions({ resultSelector, keywordOptions, mode, query, disabled, onChange, onModeChange }: {
  resultSelector?: ReactNode
  keywordOptions: boolean
  mode: SearchMode
  query: string
  disabled: boolean
  onChange: (value: string) => void
  onModeChange: (mode: SearchMode) => void
}) {
  const { t } = useTranslation()
  const broadExample = t('search.input.exampleBroad')
  const exactExample = t('search.input.exampleExact')
  // Keep the sticky row directly under the full search view, not a short form wrapper.
  return <>
    <div className={`search-options-toolbar${resultSelector ? ' search-options-toolbar-sticky' : ''}`}>
      {resultSelector}
      {keywordOptions && <fieldset className="search-mode">
        <legend>{t('search.input.modeLabel')}</legend>
        <label>
          <input type="radio" name="search-mode" checked={mode === 'all'} onChange={() => onModeChange('all')} />
          {t('search.input.allWordsMode')}
        </label>
        <label>
          <input type="radio" name="search-mode" checked={mode === 'broader'} onChange={() => onModeChange('broader')} />
          {t('search.input.broaderMode')}
        </label>
      </fieldset>}
    </div>
    <div className="search-options-content">
      <p className="search-help" id="search-input-help">
        {t(!keywordOptions ? 'search.input.helpConceptual'
          : mode === 'broader' ? 'search.input.helpBroader' : 'search.input.help')}
      </p>
      {keywordOptions && query.trim().length === 0 && (
        <div className="search-examples" aria-label={t('search.input.examplesLabel')}>
          <button type="button" className="search-example" onClick={() => onChange(broadExample)} disabled={disabled}>
            {broadExample}
          </button>
          <button type="button" className="search-example" onClick={() => onChange(exactExample)} disabled={disabled}>
            {exactExample}
          </button>
        </div>
      )}
    </div>
  </>
}
