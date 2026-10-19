import type { TFunction } from 'i18next'

const reasons = ['unavailable', 'missing', 'unsupported', 'remote', 'invalid-path', 'size-limit', 'total-limit', 'svg', 'load-failed'] as const

export function imageIssueReason(reason: string, t: TFunction): string {
  const known = reason as typeof reasons[number]
  return t(`imageIssues.reasons.${reasons.includes(known) ? known : 'unavailable'}`)
}

/** Visible, accessible placeholders use no text nodes, preserving reader text offsets. */
export function observeImageIssues(root: HTMLElement, t: TFunction): () => void {
  function show(image: HTMLImageElement, reason: string) {
    if (!root.contains(image)) return
    const placeholder = root.ownerDocument.createElement('span')
    const explanation = t('imageIssues.placeholder', { reason: imageIssueReason(reason, t) })
    const label = image.alt ? `${explanation} — ${image.alt}` : explanation
    placeholder.className = 'reader-image-error'
    placeholder.setAttribute('role', 'img')
    placeholder.setAttribute('aria-label', label)
    placeholder.dataset.message = label
    placeholder.dataset.reason = reason
    placeholder.dataset.alt = image.alt
    placeholder.dir = 'auto'
    if (image.id) placeholder.id = image.id
    image.replaceWith(placeholder)
  }
  function onError(event: Event) {
    const image = event.target
    if (image instanceof HTMLImageElement) show(image, 'load-failed')
  }
  root.addEventListener('error', onError, true)
  for (const image of root.querySelectorAll<HTMLImageElement>('img')) {
    const reason = image.getAttribute('data-papercut-image-error')
    if (reason) show(image, reason)
    else if (!image.getAttribute('src')) show(image, 'unavailable')
    else if (image.complete && image.naturalWidth === 0) show(image, 'load-failed')
  }
  // React may retain the same inner HTML when only the app language changes.
  for (const placeholder of root.querySelectorAll<HTMLElement>('.reader-image-error')) {
    const explanation = t('imageIssues.placeholder', { reason: imageIssueReason(placeholder.dataset.reason ?? '', t) })
    const label = placeholder.dataset.alt ? `${explanation} — ${placeholder.dataset.alt}` : explanation
    placeholder.dataset.message = label
    placeholder.setAttribute('aria-label', label)
  }
  return () => root.removeEventListener('error', onError, true)
}
