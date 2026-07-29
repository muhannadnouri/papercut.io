import { useTranslation } from 'react-i18next'
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
  const { t } = useTranslation()
  return (
    <div className="find-bar">
      <input
        ref={inputRef}
        type="text"
        dir="auto"
        className="find-input"
        placeholder={t('reader.findBar.placeholder')}
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
          {matchCount === 0
            ? t('reader.findBar.noMatches')
            : t('reader.findBar.matchPosition', { current: currentIndex + 1, total: matchCount })}
        </span>
      )}
      <button className="find-nav-btn" onClick={onPrev} disabled={matchCount === 0} title={t('reader.findBar.previous')} aria-label={t('reader.findBar.previous')}>&#9650;</button>
      <button className="find-nav-btn" onClick={onNext} disabled={matchCount === 0} title={t('reader.findBar.next')} aria-label={t('reader.findBar.next')}>&#9660;</button>
      <button className="find-close" onClick={onClose} aria-label={t('reader.findBar.close')}>&times;</button>
    </div>
  )
}
