/**
 * Return the hash to scroll to when a link points back into the current reader.
 *
 * EPUB rewrites ToC and footnote links into plain hashes like `#chapter-2`,
 * but imported HTML can also contain absolute same-page links. Treat those as
 * internal too so they use the reader's fixed-header-aware scroll path instead
 * of changing the app URL or showing the external-link prompt.
 */
export function getInternalDocumentHash(href: string): string | null {
  const trimmed = href.trim()
  if (!trimmed || trimmed === '#') {
    return null
  }
  if (trimmed.startsWith('#')) {
    return trimmed
  }

  try {
    const target = new URL(trimmed, window.location.href)
    const current = new URL(window.location.href)
    if (
      target.origin === current.origin &&
      target.pathname === current.pathname &&
      target.search === current.search &&
      target.hash
    ) {
      return target.hash
    }
  } catch {
    return null
  }

  return null
}

/**
 * Normalize a reader link that should leave Papercut.
 *
 * Hash links are handled separately by `getInternalDocumentHash`; active
 * schemes are ignored defensively even though backend sanitizers should already
 * remove them. Only ordinary web/contact protocols are allowed through so
 * imported documents cannot launch local files or arbitrary app URL schemes.
 */
export function getExternalLinkUrl(href: string): string | null {
  const trimmed = href.trim()
  if (!trimmed || trimmed.startsWith('#')) {
    return null
  }

  try {
    const target = new URL(trimmed, window.location.href)
    const allowedProtocols = new Set(['http:', 'https:', 'mailto:', 'tel:'])
    if (!allowedProtocols.has(target.protocol)) {
      return null
    }
    return target.href
  } catch {
    return null
  }
}
