import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode
} from 'react'
import './AppSelect.css'

export interface AppSelectOption {
  value: string
  label: string
  disabled?: boolean
  description?: ReactNode
  style?: CSSProperties
}

interface AppSelectProps {
  value: string
  options: AppSelectOption[]
  onChange: (value: string) => void
  disabled?: boolean
  ariaLabel?: string
  ariaLabelledBy?: string
  className?: string
}

export function AppSelect({
  value,
  options,
  onChange,
  disabled = false,
  ariaLabel,
  ariaLabelledBy,
  className = '',
}: AppSelectProps) {
  const listboxId = useId()
  const rootRef = useRef<HTMLDivElement | null>(null)
  const buttonRef = useRef<HTMLButtonElement | null>(null)
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([])
  const [open, setOpen] = useState(false)
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value))
  const [activeIndex, setActiveIndex] = useState(selectedIndex)
  const selectedOption = options[selectedIndex]

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: PointerEvent) {
      const root = rootRef.current
      if (!root || root.contains(event.target as Node)) return
      setOpen(false)
    }

    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  }, [open])

  useEffect(() => {
    if (!open) return
    optionRefs.current[activeIndex]?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex, open])

  function openAt(index: number) {
    setActiveIndex(enabledIndexOrFallback(options, index))
    setOpen(true)
  }

  function close() {
    setOpen(false)
    setActiveIndex(selectedIndex)
  }

  function selectOption(option: AppSelectOption) {
    if (option.disabled) return
    onChange(option.value)
    setOpen(false)
    requestAnimationFrame(() => buttonRef.current?.focus())
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (disabled) return

    if (event.key === 'Escape') {
      if (open) event.preventDefault()
      close()
      return
    }

    if (event.key === 'Tab') {
      close()
      return
    }

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      if (!open) {
        openAt(selectedIndex)
        return
      }
      setActiveIndex((current) => nextEnabledIndex(options, current, event.key === 'ArrowDown' ? 1 : -1))
      return
    }

    if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault()
      openAt(event.key === 'Home' ? 0 : options.length - 1)
      return
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      if (!open) {
        openAt(selectedIndex)
        return
      }
      const option = options[activeIndex]
      if (option) selectOption(option)
    }
  }

  return (
    <div
      className={['app-select', open ? 'app-select-open' : '', className].filter(Boolean).join(' ')}
      ref={rootRef}
      onKeyDown={handleKeyDown}
    >
      <button
        ref={buttonRef}
        type="button"
        className="app-select-button"
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listboxId : undefined}
        aria-activedescendant={open ? `${listboxId}-${activeIndex}` : undefined}
        disabled={disabled}
        onClick={() => (open ? close() : openAt(selectedIndex))}
      >
        <span className="app-select-value" style={selectedOption?.style}>{selectedOption?.label ?? value}</span>
        <span className="app-select-chevron" aria-hidden="true">▾</span>
      </button>

      {open && (
        <div className="app-select-menu" id={listboxId} role="listbox" aria-label={ariaLabel} aria-labelledby={ariaLabelledBy}>
          {options.map((option, index) => (
            <button
              key={option.value}
              id={`${listboxId}-${index}`}
              ref={(node) => { optionRefs.current[index] = node }}
              type="button"
              className={[
                'app-select-option',
                index === activeIndex ? 'app-select-option-active' : '',
                option.value === value ? 'app-select-option-selected' : '',
              ].filter(Boolean).join(' ')}
              role="option"
              aria-selected={option.value === value}
              aria-disabled={option.disabled || undefined}
              disabled={option.disabled}
              style={option.style}
              onMouseEnter={() => {
                if (!option.disabled) setActiveIndex(index)
              }}
              onClick={() => selectOption(option)}
            >
              <span className="app-select-option-label">{option.label}</span>
              {option.description && <span className="app-select-option-description">{option.description}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

function enabledIndexOrFallback(options: AppSelectOption[], index: number): number {
  if (!options.length) return 0
  if (!options[index]?.disabled) return Math.max(0, index)
  return nextEnabledIndex(options, index, 1)
}

function nextEnabledIndex(options: AppSelectOption[], current: number, direction: 1 | -1): number {
  if (!options.length) return 0
  for (let step = 1; step <= options.length; step += 1) {
    const index = (current + step * direction + options.length) % options.length
    if (!options[index].disabled) return index
  }
  return current
}
