import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import papercutIcon from '../../assets/papercut-icon.png'
import { EasterEggOverlay } from '../EasterEgg/EasterEggOverlay'
import { useEasterEgg } from '../EasterEgg/useEasterEgg'
import './AppHeader.css'

interface AppHeaderProps {
  actions?: ReactNode
}

export function AppHeader({ actions }: AppHeaderProps) {
  const { t } = useTranslation()
  const easterEgg = useEasterEgg()

  return (
    <header className="app-header">
      <div className="app-header-actions">
        {actions}
      </div>
      <h1 className="app-title">
        <img className="app-title-icon" src={papercutIcon} alt="" aria-hidden="true" onClick={easterEgg.registerTap} />
        <span>Papercut</span>
      </h1>
      <p className="app-subtitle">{t('app.subtitle')}</p>
      <EasterEggOverlay open={easterEgg.revealed} onDismiss={easterEgg.dismiss} />
    </header>
  )
}
