import './ScrollTopButton.css'

interface ScrollTopButtonProps {
  visible: boolean
  onClick: () => void
}

export function ScrollTopButton({ visible, onClick }: ScrollTopButtonProps) {
  if (!visible) return null
  return (
    <button className="scroll-top-btn" aria-label="Scroll to top" title="Top" onClick={onClick}>
      <svg className="scroll-top-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M12 19V5" />
        <path d="m5 12 7-7 7 7" />
      </svg>
    </button>
  )
}
