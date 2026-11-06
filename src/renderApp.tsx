import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './i18n'
import App from './App.tsx'
import { AppResetNotice } from './app-reset/AppResetNotice'
import { LocaleProvider } from './i18n/LocaleProvider'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <LocaleProvider>
      <App />
      <AppResetNotice />
    </LocaleProvider>
  </StrictMode>,
)
