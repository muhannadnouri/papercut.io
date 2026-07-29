import { openUrl } from '@tauri-apps/plugin-opener'

export async function openExternalUrl(url: string): Promise<void> {
  const target = new URL(url).href

  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    await openUrl(target)
    return
  }

  const opened = window.open(target, '_blank', 'noopener,noreferrer')
  if (!opened) throw new Error('Your browser blocked the external link.')
}
