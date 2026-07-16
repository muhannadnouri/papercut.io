import './FindBar.css'

interface FindBarProps {
  query: string
  matchCount: number
  currentIndex: number
  inputRef: React.RefObject<HTMLInputElement | null>
  onChange: (value: string) => void
  onNext: () => void
  onPrev: () => void
  onClose: () => void
}

export function FindBar({
  query,
  matchCount,
  currentIndex,
  inputRef,
  onChange,
  onNext,
  onPrev,
  onClose,
}: FindBarProps) {
  return (
    <div className="find-bar">
      <input
        ref={inputRef}
        type="text"
        dir="auto"
        className="find-input"
        placeholder="Find..."
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') onClose()
          else if (e.key === 'Enter' && e.shiftKey) { e.preventDefault(); onPrev() }
          else if (e.key === 'Enter') { e.preventDefault(); onNext() }
        }}
      />
      {query.trim().length > 0 && (
        <span className="find-count">
          {matchCount === 0 ? 'No matches' : `${currentIndex + 1} of ${matchCount}`}
        </span>
      )}
      <button className="find-nav-btn" onClick={onPrev} disabled={matchCount === 0} title="Previous (Shift+Enter)">&#9650;</button>
      <button className="find-nav-btn" onClick={onNext} disabled={matchCount === 0} title="Next (Enter)">&#9660;</button>
      <button className="find-close" onClick={onClose}>&times;</button>
    </div>
  )
}
