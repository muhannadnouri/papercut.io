import './index.css'
import { finishPendingAppReset } from './app-reset/appReset'

// i18n and app modules read localStorage at import time, so an interrupted erase
// must finish before they load.
void finishPendingAppReset().finally(() => import('./renderApp'))
