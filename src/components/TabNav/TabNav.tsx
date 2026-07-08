import './TabNav.css'

export type AppTab = 'search' | 'library' | 'audiobooks'

interface TabDef {
  id: AppTab
  label: string
  icon: string
}

const TABS: TabDef[] = [
  { id: 'search', label: 'Search', icon: '\u{1F50D}' },
  { id: 'library', label: 'Library', icon: '\u{1F4DA}' },
  { id: 'audiobooks', label: 'Audiobooks', icon: '\u{1F3A7}' },
]

interface TabNavProps {
  active: AppTab
  busyTabs?: Partial<Record<AppTab, boolean>>
  onChange: (tab: AppTab) => void
}

export function TabNav({ active, busyTabs = {}, onChange }: TabNavProps) {
  return (
    <nav className="tab-nav" role="tablist" aria-label="App sections">
      {TABS.map((tab) => {
        const disabled = false
        const busy = Boolean(busyTabs[tab.id])
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active === tab.id}
            aria-disabled={disabled}
            disabled={disabled}
            className={'tab-nav-item' + (active === tab.id ? ' tab-nav-item-active' : '')}
            onClick={() => { if (!disabled) onChange(tab.id) }}
          >
            <span className="tab-nav-icon" aria-hidden="true">{busy ? <span className="spinner tab-nav-spinner" /> : tab.icon}</span>
            <span className="tab-nav-label">{tab.label}</span>
          </button>
        )
      })}
    </nav>
  )
}
