export const UNCATEGORIZED = 'Uncategorized'

export function extractPageFromAnchor(url: string): number {
  const hashIdx = url.indexOf('#')
  if (hashIdx === -1) {
    return 1
  }
  const hash = url.slice(hashIdx + 1)
  const match = hash.match(/page-(\d+)/i)
  return match ? parseInt(match[1], 10) : 1
}

export function deriveAuthor(url: string): string {
  // if (/^\/(uploads)\//.test(url)) return 'User Uploads'
  if (/^\/(uploads|user-uploads)\//.test(url)) {
    return 'User Uploads'
  }

  const idx = url.indexOf('/documents/')
  if (idx === -1) {
    return UNCATEGORIZED
  }
  const tail = url.slice(idx + '/documents/'.length)
  const cleaned = tail.split('?')[0].split('#')[0]
  const slashIdx = cleaned.indexOf('/')
  if (slashIdx === -1) {
    return UNCATEGORIZED
  }
  try {
    return decodeURIComponent(cleaned.slice(0, slashIdx))
  } catch {
    return cleaned.slice(0, slashIdx)
  }
}
