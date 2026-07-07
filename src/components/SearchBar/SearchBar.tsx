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
    </div>
  )
}
