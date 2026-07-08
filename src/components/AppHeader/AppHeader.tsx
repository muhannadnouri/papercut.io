import type { ReactNode } from 'react'
import papercutIcon from '../../assets/papercut-icon.png'
import './AppHeader.css'

interface AppHeaderProps {
  actions?: ReactNode
}

export function AppHeader({ actions }: AppHeaderProps) {
  return (
    <header className="app-header">
      <div className="app-header-actions">
        {actions}
      </div>
      <h1 className="app-title">
        <img className="app-title-icon" src={papercutIcon} alt="" aria-hidden="true" />
        <span>Papercut</span>
      </h1>
      <p className="app-subtitle">Search, Read, & Listen Offline</p>
    </header>
  )
}
