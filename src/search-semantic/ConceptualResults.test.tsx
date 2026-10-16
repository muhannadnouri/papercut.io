// @vitest-environment jsdom
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ConceptualResults } from './ConceptualResults'
import type { ConceptualResponse, ConceptualState } from './useConceptualSearch'

vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string, values?: Record<string, unknown>) => `${key}${values ? JSON.stringify(values) : ''}`,
}) }))
const response: ConceptualResponse = {
  availability: 'ready', indexedDocuments: 1, eligibleDocuments: 1, scannedPassages: 2,
  results: [{ documentId: 'guide', url: '/uploads/guide.pdf', format: 'pdf', title: 'Guide <b>title</b>',
    context: [], provenance: 'conceptual', score: 0.8, sectionIndex: 3, pageIndex: 3, sectionTitle: 'Connections',
    byteStart: 120, byteEnd: 180, text: '🙂 Electric buses <mark>run</mark> & connect.' }],
}
function html(state: ConceptualState, openingDisabled = false) {
  return renderToStaticMarkup(<ConceptualResults state={state} openingDisabled={openingDisabled} onViewDocument={vi.fn()} />)
}
let root: Root
let container: HTMLDivElement
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
})
afterEach(async () => {
  await act(() => root.unmount())
  container.remove()
  vi.unstubAllGlobals()
})
async function render(state: ConceptualState, onViewDocument = vi.fn(), openingDisabled = false) {
  await act(() => root.render(<ConceptualResults state={state} onViewDocument={onViewDocument} openingDisabled={openingDisabled} />))
}
async function click(element: HTMLElement) { await act(() => element.click()) }

it('keeps the default-off path silent and distinguishes loading, empty and all fallback reasons', () => {
  expect(html(null)).toBe('')
  expect(html({ phase: 'loading' })).toContain('aria-busy="true"')
  for (const reason of ['unavailable', 'preparing', 'missing-index', 'incompatible-index', 'partial', 'failed'] as const) {
    const value = html({ phase: 'fallback', reason, response: { ...response, availability: 'partial', eligibleDocuments: 2 } })
    expect(value).toContain(`search.conceptual.${reason}`)
    expect(value).not.toContain('result-card-primary')
    if (reason === 'partial') expect(value).toContain('&quot;total&quot;:2')
  }
  expect(html({ phase: 'ready', response: { ...response, results: [] } })).toContain('search.conceptual.empty')
})

it('escapes authored text, labels the channel separately and avoids scores, literal marks/counts and query phrase targets', async () => {
  const state = { phase: 'ready', response } as const
  const value = html(state)
  expect(value).toContain('search.conceptual.title')
  expect(value).not.toContain('search.conceptual.description')
  expect(value).toContain('&lt;mark&gt;run&lt;/mark&gt; &amp; connect.')
  expect(value).not.toContain('<mark>')
  expect(value).not.toContain('exactMatch')
  expect(value).not.toContain('0.8')
  expect(html(state, true)).toContain('disabled=""')
  const onViewDocument = vi.fn()
  await render(state, onViewDocument)
  await click(container.querySelector<HTMLButtonElement>('.result-card-primary')!)
  expect(onViewDocument).toHaveBeenCalledWith('/uploads/guide.pdf', {
    text: response.results[0].text, sectionIndex: 3, pageIndex: 3,
  })
  expect(onViewDocument.mock.calls[0][1]).not.toHaveProperty('byteStart')
})

it('counts displayed documents, hides healthy coverage and retains partial fallback without permanent matching help', () => {
  const ready = html({ phase: 'ready', response: { ...response, indexedDocuments: 10, eligibleDocuments: 10 } })
  expect(ready).toContain('search.conceptual.shown{&quot;count&quot;:1}')
  expect(ready).not.toContain('search.conceptual.coverage')
  expect(ready).not.toContain('search.conceptual.matchingDetails')
  const empty = html({ phase: 'ready', response: { ...response, results: [] } })
  expect(empty).toContain('search.conceptual.shown{&quot;count&quot;:0}')
  expect(empty).not.toContain('search.conceptual.coverage')
  const partial = html({ phase: 'fallback', reason: 'partial', response: { ...response, availability: 'partial', eligibleDocuments: 2 } })
  expect(partial).toContain('search.conceptual.coverage{&quot;ready&quot;:1,&quot;total&quot;:2}')
  expect(partial).toContain('search.conceptual.partial')
  expect(partial).not.toContain('search.conceptual.shown')
  expect(partial).not.toContain('result-card-primary')
  expect(html({ phase: 'loading' })).not.toContain('search.conceptual.shown')
})

it('opens escaped context through original reader targets outside the preview button', async () => {
  const neighbor = { sectionIndex: 4, pageIndex: 4, sectionTitle: 'Cautions',
    byteStart: 3, byteEnd: 60, text: '港 Keep <script>unsafe()</script> as source text.' }
  const state: ConceptualState = { phase: 'ready', response: { ...response,
    results: [{ ...response.results[0], context: [neighbor] }] } }
  const value = html(state)
  expect(value).toContain('</button><details class="result-explore conceptual-context">')
  expect(value).toContain('search.conceptual.contextHelp')
  expect(value).toContain('search.conceptual.matchedPassage')
  expect(value).toContain('search.conceptual.nearbyText')
  expect(value).toContain('&lt;script&gt;unsafe()&lt;/script&gt;')
  expect(value).not.toContain('<script>')
  expect(html(state, true).match(/disabled=""/g)).toHaveLength(3)
  const onViewDocument = vi.fn()
  await render(state, onViewDocument)
  await click(container.querySelector('summary')!)
  expect(container.querySelector('details')!.open).toBe(true)
  for (const button of container.querySelectorAll<HTMLButtonElement>('.result-card-primary')) await click(button)
  expect(onViewDocument).toHaveBeenLastCalledWith('/uploads/guide.pdf', {
    text: neighbor.text, sectionIndex: 4, pageIndex: 4,
  })
  expect(onViewDocument).toHaveBeenCalledTimes(3)
})


function surroundingResult() {
  const result = response.results[0]
  const context = [
    { ...result, sectionIndex: 5, byteStart: 0, byteEnd: 30, sectionTitle: 'Far After', text: 'Far after source' },
    { ...result, sectionIndex: 0, byteStart: 0, byteEnd: 30, sectionTitle: 'Far Before', text: 'Far before source' },
    { ...result, byteStart: 200, byteEnd: 240, sectionTitle: 'Closest After', text: 'Closest after source' },
    { ...result, sectionIndex: 1, byteStart: 0, byteEnd: 30, sectionTitle: 'Earlier', text: 'Earlier source' },
    { ...result, byteStart: 30, byteEnd: 90, sectionTitle: 'Closest Before', text: 'Closest before source' },
    { ...result, sectionIndex: 4, byteStart: 0, byteEnd: 30, sectionTitle: 'Later', text: 'Later source' },
  ]
  return { ...result, context }
}
function headings(card: ParentNode = container) {
  return [...card.querySelectorAll('.conceptual-context-block button > .result-meta')].map(node => node.textContent)
}

it('shows the nearest three blocks, expands in source order and collapses without losing control focus or reader targets', async () => {
  const result = surroundingResult()
  const original = JSON.stringify(result)
  const onViewDocument = vi.fn()
  await render({ phase: 'ready', response: { ...response, results: [result] } }, onViewDocument)
  await click(container.querySelector('summary')!)
  expect(headings()).toEqual(['Closest Before', 'Connections', 'Closest After'])
  expect(container.querySelector('.conceptual-context-block-matched')!.textContent).toContain('Connections')
  const more = container.querySelector<HTMLButtonElement>('.result-evidence-action')!
  expect(more.textContent).toContain('search.conceptual.showMore{"count":4}')
  const icon = more.firstElementChild!
  expect(icon.tagName.toLowerCase()).toBe('svg')
  expect(icon.getAttribute('aria-hidden')).toBe('true')
  expect(icon.getAttribute('focusable')).toBe('false')
  const collapsedIcon = icon.querySelector('path')!.getAttribute('d')
  expect(more.getAttribute('aria-expanded')).toBe('false')
  expect(document.getElementById(more.getAttribute('aria-controls')!)).toBe(container.querySelector('.conceptual-context-previews'))
  more.focus()
  await act(() => { icon.querySelector('path')!.dispatchEvent(new MouseEvent('click', { bubbles: true })) })
  expect(headings()).toEqual(['Far Before', 'Earlier', 'Closest Before', 'Connections', 'Closest After', 'Later', 'Far After'])
  expect(more.getAttribute('aria-expanded')).toBe('true')
  expect(more.textContent).toContain('search.conceptual.showLess')
  expect(more.querySelector('path')!.getAttribute('d')).not.toBe(collapsedIcon)
  expect(document.activeElement).toBe(more)
  await click(container.querySelector<HTMLButtonElement>('.conceptual-context-block button')!)
  expect(onViewDocument).toHaveBeenLastCalledWith(result.url, {
    text: 'Far before source', sectionIndex: 0, pageIndex: 3,
  })
  await click(more)
  expect(more.querySelector('path')!.getAttribute('d')).toBe(collapsedIcon)
  expect(headings()).toEqual(['Closest Before', 'Connections', 'Closest After'])
  expect(document.activeElement).toBe(more)
  expect(container.querySelector('.conceptual-context-previews')!.textContent).not.toContain('Far before source')
  expect(JSON.stringify(result)).toBe(original)
  await render({ phase: 'ready', response: { ...response, results: [result] } }, onViewDocument, true)
  for (const button of container.querySelectorAll<HTMLButtonElement>('.result-card-primary')) expect(button.disabled).toBe(true)
})

it('loads the next document batch from its leading icon and keeps the exhausted action guarded', async () => {
  const results = Array.from({ length: 11 }, (_, index) => ({ ...response.results[0], documentId: `guide-${index}` }))
  const onShowMore = vi.fn()
  const renderBatch = (visibleLimit: number) => act(() => root.render(<ConceptualResults
    state={{ phase: 'ready', response: { ...response, results } }} visibleLimit={visibleLimit}
    onShowMore={onShowMore} onViewDocument={vi.fn()} />))
  await renderBatch(10)
  const more = container.querySelector<HTMLButtonElement>('.result-list-more button')!
  expect(more.textContent).toBe('search.results.showMore')
  expect(more.firstElementChild?.tagName.toLowerCase()).toBe('svg')
  expect(more.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true')
  expect(more.querySelector('svg')?.getAttribute('focusable')).toBe('false')
  await act(() => { more.querySelector('path')!.dispatchEvent(new MouseEvent('click', { bubbles: true })) })
  expect(onShowMore).toHaveBeenCalledOnce()
  expect(container.querySelectorAll('article')).toHaveLength(10)
  await renderBatch(11)
  expect(container.querySelectorAll('article')).toHaveLength(11)
  expect(more.getAttribute('aria-disabled')).toBe('true')
  await act(() => { more.querySelector('path')!.dispatchEvent(new MouseEvent('click', { bubbles: true })) })
  expect(onShowMore).toHaveBeenCalledOnce()
})

it('keeps expansion local to each card and resets it for a different matched source span', async () => {
  const result = surroundingResult()
  const second = { ...result, documentId: 'second', url: '/uploads/second.pdf' }
  await render({ phase: 'ready', response: { ...response, results: [result, second] } })
  const cards = container.querySelectorAll('article')
  const firstMore = cards[0].querySelector<HTMLButtonElement>('.result-evidence-action')!
  await click(firstMore)
  expect(headings(cards[0])).toHaveLength(7)
  expect(headings(cards[1])).toHaveLength(3)
  expect(cards[1].querySelector('.result-evidence-action')!.getAttribute('aria-expanded')).toBe('false')
  const moved = { ...result, byteStart: 100, byteEnd: 120 }
  await render({ phase: 'ready', response: { ...response, results: [moved, second] } })
  expect(headings(container.querySelector('article')!)).toHaveLength(3)
  expect(container.querySelector('.result-evidence-action')!.getAttribute('aria-expanded')).toBe('false')
})

it('reflows bounded Unicode PDF previews while opening the untouched source, including text beyond the preview', async () => {
  const text = '🙂 Straightfor\u2010\r\nward prose flows\r\nacross lines.\r\n\r\nSecond paragraph has ‘quotes’ and high-availability. '
    + 'Long authored sentence 🙂 '.repeat(30) + 'Original source tail.'
  const result = { ...response.results[0], text }
  const original = JSON.stringify(result)
  const onViewDocument = vi.fn()
  await render({ phase: 'ready', response: { ...response, results: [result] } }, onViewDocument)
  const preview = container.querySelector('.conceptual-context-text')!.textContent!
  expect(preview).toContain('🙂 Straightforward prose flows across lines.\n\nSecond paragraph')
  expect(preview).toContain('‘quotes’ and high-availability')
  expect(preview).not.toContain('Original source tail.')
  expect(preview.endsWith('…')).toBe(true)
  expect([...preview].length).toBeLessThanOrEqual(401)
  expect([...preview].some(char => char.length === 1 && /[\uD800-\uDFFF]/u.test(char))).toBe(false)
  await click(container.querySelector<HTMLButtonElement>('.conceptual-context-block button')!)
  expect(onViewDocument).toHaveBeenCalledWith(result.url, {
    text, sectionIndex: result.sectionIndex, pageIndex: result.pageIndex,
  })
  expect(JSON.stringify(result)).toBe(original)
})

it.each(['html', 'epub'])('preserves authored %s line breaks and has no redundant expansion control for a small context', async format => {
  const result = { ...response.results[0], format, text: 'Authored line-\nbreak.\n\nSeparate paragraph.' }
  await render({ phase: 'ready', response: { ...response, results: [result] } })
  expect(container.querySelector('.conceptual-context-text')!.textContent).toBe(result.text)
  expect(container.querySelector('.result-evidence-action')).toBeNull()
  expect(headings()).toEqual(['Connections'])
})

it.each(['-', '‐', '‑'])('joins PDF word wraps using %s without joining across paragraphs or rewriting inline punctuation', async hyphen => {
  const text = `Straightfor${hyphen}\nward.\n\nKeep${hyphen}\n\nSeparate paragraph. high-availability\nworks. lower-\nUpper stays distinct.`
  const result = { ...response.results[0], text }
  await render({ phase: 'ready', response: { ...response, results: [result] } })
  expect(container.querySelector('.conceptual-context-text')!.textContent).toBe(
    `Straightforward.\n\nKeep${hyphen}\n\nSeparate paragraph. high-availability works. lower- Upper stays distinct.`,
  )
})

it('bounds previews without whitespace and retains the complete original source for navigation', async () => {
  const text = '港🙂'.repeat(300)
  const result = { ...response.results[0], text }
  const onViewDocument = vi.fn()
  await render({ phase: 'ready', response: { ...response, results: [result] } }, onViewDocument)
  const preview = container.querySelector('.conceptual-context-text')!.textContent!
  expect(preview).toBe('港🙂'.repeat(200) + '…')
  await click(container.querySelector<HTMLButtonElement>('.conceptual-context-block button')!)
  expect(onViewDocument).toHaveBeenCalledWith(result.url, {
    text, sectionIndex: result.sectionIndex, pageIndex: result.pageIndex,
  })
})
