import { useTranslation } from 'react-i18next'
import './TabNav.css'

export type AppTab = 'search' | 'library' | 'audiobooks'

interface TabDef {
  id: AppTab
  labelKey: 'navigation.search' | 'navigation.library' | 'navigation.audiobooks'
  icon: string
}

const TABS: TabDef[] = [
  { id: 'search', labelKey: 'navigation.search', icon: '\u{1F50D}' },
  { id: 'library', labelKey: 'navigation.library', icon: '\u{1F4DA}' },
  { id: 'audiobooks', labelKey: 'navigation.audiobooks', icon: '\u{1F3A7}' },
]

interface TabNavProps {
  active: AppTab
  busyTabs?: Partial<Record<AppTab, boolean>>
  onChange: (tab: AppTab) => void
}

export function TabNav({ active, busyTabs = {}, onChange }: TabNavProps) {
  const { t } = useTranslation()

  return (
    <nav className="tab-nav" role="tablist" aria-label={t('navigation.label')}>
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
            <span className="tab-nav-label">{t(tab.labelKey)}</span>
          </button>
        )
      })}
    </nav>
  )
}
