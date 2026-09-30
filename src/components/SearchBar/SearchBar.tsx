import { useTranslation } from 'react-i18next'
import type { SearchQueryError } from '../../hooks/useSearch'
import type { SearchMode } from '../../types/search'
import './SearchBar.css'

interface SearchBarProps {
  mode: SearchMode
  query: string
  queryError: SearchQueryError | null
  disabled: boolean
  loading: boolean
  submittedQuery: string
  onChange: (value: string) => void
  onModeChange: (mode: SearchMode) => void
  onSubmit: () => void
}

export function SearchBar({
  mode,
  query,
  queryError,
  disabled,
  loading,
  submittedQuery,
  onChange,
  onModeChange,
  onSubmit,
}: SearchBarProps) {
  const { t } = useTranslation()
  const broadExample = t('search.input.exampleBroad')
  const exactExample = t('search.input.exampleExact')
  const searching = loading && query.trim().toLowerCase() === submittedQuery.toLowerCase()
  const submitDisabled = disabled || searching || query.trim().length === 0

  return (
    <div className="search-container">
      <div className="search-row">
        <input
          type="text"
          dir="auto"
          className="search-input"
          aria-label={t('search.input.placeholder')}
          aria-describedby={`search-input-help${queryError ? ' search-input-error' : ''}`}
          aria-invalid={queryError ? true : undefined}
          placeholder={disabled ? t('search.input.loadingPlaceholder') : t('search.input.placeholder')}
          value={query}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !submitDisabled) { e.preventDefault(); onSubmit() }
          }}
          disabled={disabled}
          autoFocus
        />
        <button
          className="search-btn"
          onClick={onSubmit}
          disabled={submitDisabled}
          aria-busy={searching || undefined}
        >
          {searching
            ? <span className="spinner search-btn-spinner" aria-hidden="true" />
            : (
                <svg className="search-btn-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                  <circle cx="11" cy="11" r="7" />
                  <path d="m16 16 5 5" />
                </svg>
              )}
          {t('search.input.button')}
        </button>
      </div>
      <fieldset className="search-mode">
        <legend>{t('search.input.modeLabel')}</legend>
        <label>
          <input type="radio" name="search-mode" checked={mode === 'all'} onChange={() => onModeChange('all')} />
          {t('search.input.allWordsMode')}
        </label>
        <label>
          <input type="radio" name="search-mode" checked={mode === 'broader'} onChange={() => onModeChange('broader')} />
          {t('search.input.broaderMode')}
        </label>
      </fieldset>
      <p className="search-help" id="search-input-help">
        {t(mode === 'broader' ? 'search.input.helpBroader' : 'search.input.help')}
      </p>
      {queryError && (
        <p className="search-input-error" id="search-input-error" role="alert">
          {t(`search.input.${queryError}`)}
        </p>
      )}
      {query.trim().length === 0 && (
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
  )
}
