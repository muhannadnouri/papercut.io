import { isValidElement, type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it, vi } from 'vitest'
import { ConceptualResults } from './ConceptualResults'
import type { ConceptualResponse, ConceptualState } from './useConceptualSearch'

vi.mock('react', async (original) => ({ ...await original<typeof import('react')>(), useId: () => 'conceptual-title' }))
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
function click(node: ReactNode): boolean {
  if (Array.isArray(node)) return node.some(click)
  if (!isValidElement<{ children?: ReactNode; onClick?: () => void }>(node)) return false
  if (node.type === 'button') { node.props.onClick!(); return true }
  return click(node.props.children)
}

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

it('escapes authored text, labels the channel separately and avoids scores, literal marks/counts and query phrase targets', () => {
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
  expect(click(ConceptualResults({ state, onViewDocument }))).toBe(true)
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

it('expands unranked context with full escaped text and independent reader links outside the preview button', () => {
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
  function clickAll(node: ReactNode) {
    if (Array.isArray(node)) { node.forEach(clickAll); return }
    if (!isValidElement<{ children?: ReactNode; onClick?: () => void }>(node)) return
    if (node.type === 'button') node.props.onClick!()
    clickAll(node.props.children)
  }
  clickAll(ConceptualResults({ state, onViewDocument }))
  expect(onViewDocument).toHaveBeenLastCalledWith('/uploads/guide.pdf', {
    text: neighbor.text, sectionIndex: 4, pageIndex: 4,
  })
  expect(onViewDocument).toHaveBeenCalledTimes(3)
})
