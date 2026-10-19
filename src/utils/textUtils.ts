export const SEARCH_DASH_CHARACTERS = ['-', '‐', '‑', '‒', '–', '—', '―'] as const

const SEARCH_PUNCTUATION: Readonly<Record<string, string>> = {
  '‘': "'",
  '’': "'",
  '“': '"',
  '”': '"',
  '‐': '-',
  '‑': '-',
  '‒': '-',
  '–': '-',
  '—': '-',
  '―': '-',
}

export function normalizeSearchCharacter(character: string): string {
  return SEARCH_PUNCTUATION[character] ?? character
}

export function normalizeSearchPunctuation(s: string): string {
  return s.replace(/[‘’“”‐‑‒–—―]/g, normalizeSearchCharacter)
}

export function normalizeForDisplay(s: string): string {
  return normalizeSearchPunctuation(s)
    .replace(/\s+/g, ' ')
    .trim()
}

export function normalizeForPhraseMatch(s: string): string {
  return normalizeForDisplay(s).toLowerCase()
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/** Escape native snippet text while retaining its only supported markup. */
export function sanitizeMarkedExcerpt(s: string): string {
  return escapeHtml(s)
    .replace(/&lt;mark&gt;/g, '<mark>')
    .replace(/&lt;\/mark&gt;/g, '</mark>')
}

export function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
