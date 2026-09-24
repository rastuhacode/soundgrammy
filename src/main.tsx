import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from '@/App'
import { installGlobalErrorLogging } from '@/lib/app-logger'
import { isAndroid } from '@/lib/platform'
import '@/index.css'

installGlobalErrorLogging()
if (isAndroid(navigator.userAgent)) document.documentElement.dataset.platform = 'android'

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
