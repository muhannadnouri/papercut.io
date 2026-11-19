import type { OutlineEntry } from '../../viewers/types'

/** EPUB type is a literal colon-named attribute on the sanitized HTML surface. */
function inNavigation(heading: HTMLElement, article: HTMLElement): boolean {
  for (let node: HTMLElement | null = heading; node && node !== article; node = node.parentElement) {
    if (node.matches('nav, [role~="navigation"], [role~="doc-toc"]') || node.getAttribute('epub:type')?.split(/\s+/).includes('toc')) {
      return true
    }
  }
  return false
}

/** Read existing section ordinals from the mounted article. Counting usable headings
 * before returning the tree lets a single top-level heading with children qualify. */
export function buildHtmlOutline(article: HTMLElement): OutlineEntry[] {
  const roots: OutlineEntry[] = []
  const parents: {
    rank: number
    entry: OutlineEntry
  }[] = []
  let count = 0
  for (const heading of article.querySelectorAll<HTMLElement>('h1,h2,h3,h4,h5,h6')) {
    const target = heading.dataset.papercutSection
    const ordinal = Number(target)
    if (target !== String(ordinal) || !Number.isSafeInteger(ordinal) || ordinal < 0 || inNavigation(heading, article)) {
      continue
    }
    const label = heading.textContent?.replace(/\s+/g, ' ').trim()
    if (!label) {
      continue
    }
    const rank = Number(heading.tagName.slice(1))
    while (parents.length && parents[parents.length - 1].rank >= rank) {
      parents.pop()
    }
    const entry: OutlineEntry = {
      label,
      target,
      children: [],
    }
    const siblings = parents.at(-1)?.entry.children ?? roots
    siblings.push(entry)
    parents.push({
      rank,
      entry,
    })
    count++
  }
  return count >= 2 ? roots : []
}
