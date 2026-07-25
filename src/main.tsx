import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import './i18n'
import App from './App.tsx'
import { LocaleProvider } from './i18n/LocaleProvider'
import { PdfViewerSpike } from './viewers/PdfViewerSpike'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {new URLSearchParams(window.location.search).has('pdf-spike')
      ? <PdfViewerSpike />
      : (
          <LocaleProvider>
            <App />
          </LocaleProvider>
        )}
  </StrictMode>,
)
