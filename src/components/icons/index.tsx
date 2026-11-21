type IconProps = { className?: string }

export function SearchIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="11" cy="11" r="7" />
      <path d="m16 16 5 5" />
    </svg>
  )
}

export function ConceptualIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="5" cy="12" r="3" />
      <circle cx="18" cy="5" r="3" />
      <circle cx="18" cy="19" r="3" />
      <path d="m8 10.5 7-4m-7 7 7 4" />
    </svg>
  )
}

export function ChevronDownIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <path d="m4 6 4 4 4-4" />
    </svg>
  )
}

export function OutlineIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M9 6h11M9 12h11M9 18h11" />
      <circle cx="4" cy="6" r="1" />
      <circle cx="4" cy="12" r="1" />
      <circle cx="4" cy="18" r="1" />
    </svg>
  )
}

export function BookIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 6C9 4 5 4 2 5v14c3-1 7-1 10 1 3-2 7-2 10-1V5c-3-1-7-1-10 1Zm0 0v14" />
    </svg>
  )
}

export function DocumentIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M5 3h9l5 5v13H5Zm9 0v6h5M8 13h8M8 17h6" />
    </svg>
  )
}

export function MoreActionsIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="5" r="2" />
      <circle cx="12" cy="12" r="2" />
      <circle cx="12" cy="19" r="2" />
    </svg>
  )
}
