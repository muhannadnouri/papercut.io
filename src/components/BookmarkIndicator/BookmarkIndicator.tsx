import './BookmarkIndicator.css'

interface BookmarkIndicatorProps {
  label: string
  className?: string
}

/** Passive Library status marker for documents with an explicit reader bookmark. */
export function BookmarkIndicator({ label, className = '' }: BookmarkIndicatorProps) {
  return (
    <span
      className={`bookmark-indicator ${className}`.trim()}
      role="img"
      aria-label={label}
      title={label}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M6 4.75A2.75 2.75 0 0 1 8.75 2h6.5A2.75 2.75 0 0 1 18 4.75V21l-6-3.5L6 21z" />
      </svg>
    </span>
  )
}
