import './SearchBar.css'

interface SearchBarProps {
  query: string
  disabled: boolean
  onChange: (value: string) => void
  onSubmit: () => void
}

export function SearchBar({ query, disabled, onChange, onSubmit }: SearchBarProps) {
  return (
    <div className="search-container">
      <div className="search-row">
        <input
          type="text"
          className="search-input"
          placeholder={disabled ? 'Loading Search Index...' : 'Search Documents...'}
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
          Search
        </button>
      </div>
      <p className="search-help">
        Search by words broadly, or use quotes for an exact phrase.
      </p>
      <div className="search-examples" aria-label="Search examples">
        <button type="button" className="search-example" onClick={() => onChange('green gables')} disabled={disabled}>
          green gables
        </button>
        <button type="button" className="search-example" onClick={() => onChange('"Anne Shirley"')} disabled={disabled}>
          &quot;Anne Shirley&quot;
        </button>
      </div>
    </div>
  )
}
