/** Detect iOS and iPadOS, including iPads that request a desktop user agent. */
export function isIOSWebKit(): boolean {
  if (typeof navigator === 'undefined') return false
  return /iP(ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
}

/** Detect the mobile browser platforms supported by Papercut's Tauri builds. */
export function isMobileUserAgent(): boolean {
  if (typeof navigator === 'undefined') return false
  return /Android/.test(navigator.userAgent) || isIOSWebKit()
}
