import { useTranslation } from 'react-i18next'
import './SearchBar.css'

interface SearchBarProps {
  query: string
  disabled: boolean
  onChange: (value: string) => void
  onSubmit: () => void
}

export function SearchBar({ query, disabled, onChange, onSubmit }: SearchBarProps) {
  const { t } = useTranslation()
  const broadExample = t('search.input.exampleBroad')
  const exactExample = t('search.input.exampleExact')

  return (
    <div className="search-container">
      <div className="search-row">
        <input
          type="text"
          dir="auto"
          className="search-input"
          placeholder={disabled ? t('search.input.loadingPlaceholder') : t('search.input.placeholder')}
          value={query}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); onSubmit() }
          }}
          disabled={disabled}
          autoFocus
        />
        <button
          className="search-btn"
          onClick={onSubmit}
          disabled={disabled || query.trim().length === 0}
        >
          <svg className="search-btn-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="11" cy="11" r="7" />
            <path d="m16 16 5 5" />
          </svg>
          {t('search.input.button')}
        </button>
      </div>
      <p className="search-help">
        {t('search.input.help')}
      </p>
      <div className="search-examples" aria-label={t('search.input.examplesLabel')}>
        <button type="button" className="search-example" onClick={() => onChange(broadExample)} disabled={disabled}>
          {broadExample}
        </button>
        <button type="button" className="search-example" onClick={() => onChange(exactExample)} disabled={disabled}>
          {exactExample}
        </button>
      </div>
    </div>
  )
}
