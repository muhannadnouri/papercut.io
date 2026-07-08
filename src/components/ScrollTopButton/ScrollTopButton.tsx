import './ScrollTopButton.css'

interface ScrollTopButtonProps {
  visible: boolean
  onClick: () => void
}

export function ScrollTopButton({ visible, onClick }: ScrollTopButtonProps) {
  if (!visible) return null
  return (
    <button className="scroll-top-btn" onClick={onClick}>
      &uarr; Top
    </button>
  )
}
