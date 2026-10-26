// @vitest-environment jsdom
import {
  act,
  useState,
  type ComponentProps,
} from 'react'
import {
  createRoot,
  type Root,
} from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import {
  SearchTab,
  type SearchResultView,
} from './SearchTab'
import { readFileSync } from 'node:fs'
import type { ConceptualResponse } from '../../search-semantic/useConceptualSearch'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => `${key}${values ? JSON.stringify(values) : ''}`,
    i18n: {
      language: 'en',
      resolvedLanguage: 'en',
    },
  }),
}))

const response: ConceptualResponse = {
  availability: 'ready',
  indexedDocuments: 23,
  eligibleDocuments: 23,
  scannedPassages: 230,
  results: Array.from({ length: 23 }, (_, i) => ({
    documentId: `guide-${i}`,
    url: `/uploads/guide-${i}.pdf`,
    format: 'pdf',
    title: `Concept ${i}`,
    context: Array.from({ length: 4 }, (_, j) => ({
      sectionIndex: j,
      pageIndex: j,
      sectionTitle: null,
      byteStart: j * 100,
      byteEnd: j * 100 + 90,
      text: `Context ${j}`,
    })),
    provenance: 'conceptual',
    score: 0.8,
    sectionIndex: 2,
    pageIndex: 2,
    sectionTitle: 'Connections',
    byteStart: 290,
    byteEnd: 450,
    text: 'Original full source 🙂 straightfor‐\nward passage.',
  })),
}
function props(): ComponentProps<typeof SearchTab> {
  return {
    semanticEnabled: true,
    semanticStatus: 'ready',
    hasStoredIndex: true,
    onOpenSearchSettings: vi.fn(),
    conceptualState: {
      phase: 'ready',
      response,
    },
    resultView: 'keyword',
    searchRequest: {
      revision: 1,
      scope: '*',
    },
    onResultViewChange: vi.fn(),
    onViewConceptualResult: vi.fn(),
    mode: 'all',
    collapsedAuthors: new Set(),
    disabled: false,
    docFilterLower: '',
    documentFilter: '',
    filterTitleByUrl: new Map(),
    groupedDocs: [],
    libraryOrganization: {
      folders: [],
      documentLocations: [],
    },
    loading: false,
    searchFailed: false,
    searchableDocumentCount: 200,
    searchPhase: null,
    openingDisabled: false,
    query: 'connections',
    queryError: null,
    results: Array.from({ length: 23 }, (_, i) => ({
      id: `guide-${i}`,
      url: `/uploads/guide-${i}.pdf`,
      meta: { title: `Keyword ${i}` },
      source: 'upload',
      excerpt: '<mark>connections</mark> to source',
      sectionIndex: 2,
      pageIndex: 2,
      matchCount: 1,
      matchingSections: 2,
      passages: [{
        sectionIndex: 3,
        excerpt: 'Further connections',
      }],
    })),
    selectedFilters: new Set(),
    scopeMode: 'include',
    scopeUrls: new Set(),
    scopeActive: false,
    submittedQuery: 'connections',
    lastSearchInfo: {
      mode: 'all',
      phrases: [],
      unquotedText: 'connections',
      uploadedDocuments: 23,
      uploadedMatchingSections: 46,
      starterDocuments: 0,
    },
    onChangeQuery: vi.fn(),
    onModeChange: vi.fn(),
    onClearFilters: vi.fn(),
    onScopeModeChange: vi.fn(),
    onFilterChange: vi.fn(),
    onSubmitSearch: vi.fn(),
    onToggleAllInGroup: vi.fn(),
    onToggleAuthor: vi.fn(),
    onToggleFilter: vi.fn(),
    onViewResult: vi.fn(),
  }
}
function Harness({ value }: { value: ComponentProps<typeof SearchTab> }) {
  // App owns the selected view, independently of either request's completion.
  const [view, setView] = useState<SearchResultView>('keyword')
  return <SearchTab {...value} resultView={view} onResultViewChange={next => {
    value.onResultViewChange(next)
    setView(next)
  }} />
}
let root: Root
let container: HTMLDivElement
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  // jsdom has no CSS.escape; supply the platform selector escape for generated IDs.
  vi.stubGlobal('CSS', { escape: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, character => `\\${character}`) })
  vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
})
afterEach(async () => {
  await act(() => root.unmount())
  container.remove()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})
async function render(value: ComponentProps<typeof SearchTab>) {
  await act(() => root.render(<Harness value={value} />))
}
async function click(node: HTMLElement) {
  await act(() => node.click())
}
function tab(view: SearchResultView) {
  return [...container.querySelectorAll<HTMLElement>('[role="tab"]')]
    .find(node => node.textContent!.startsWith(`search.results.${view}`))!
}
function panel(view: SearchResultView) {
  return container.querySelectorAll<HTMLElement>('.search-result-panel')[view === 'keyword' ? 0 : 1]
}
function more(view: SearchResultView) {
  return panel(view).querySelector<HTMLButtonElement>('.result-list-more button')!
}

it('keeps semantic-off search bounded with no result tabs or conceptual notices', async () => {
  const value = props()
  await render({
    ...value,
    semanticEnabled: false,
    conceptualState: {
      phase: 'fallback',
      reason: 'failed',
    },
  })
  expect(container.querySelector('[role="tablist"]')).toBeNull()
  expect(container.querySelectorAll('.result-card')).toHaveLength(10)
  expect(container.textContent).not.toContain('search.conceptual.failed')
  await click(container.querySelector('.result-list-more button')!)
  expect(container.querySelectorAll('.result-card')).toHaveLength(20)
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
})

it('uses linked tabs with one active focus target, hides inactive evidence and scopes keyword options', async () => {
  const value = props()
  await render(value)
  expect(tab('keyword').getAttribute('aria-selected')).toBe('true')
  expect(tab('keyword').tabIndex).toBe(0)
  expect(tab('conceptual').tabIndex).toBe(-1)
  expect(panel('keyword').getAttribute('aria-labelledby')).toBe(tab('keyword').id)
  expect(panel('conceptual').hasAttribute('inert')).toBe(true)
  expect(panel('conceptual').hidden).toBe(true)
  expect(container.querySelectorAll('input[name="search-mode"]')).toHaveLength(2)
  for (const view of ['keyword', 'conceptual'] as const) {
    const icon = tab(view).querySelector('.search-result-tab-icon')!
    const label = tab(view).querySelector('span[dir="auto"]')!
    expect(icon.getAttribute('aria-hidden')).toBe('true')
    expect(icon.getAttribute('focusable')).toBe('false')
    expect(icon.compareDocumentPosition(label) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  }
  await act(() => tab('conceptual').querySelector('.search-result-tab-icon')!.dispatchEvent(new MouseEvent('click', { bubbles: true })))
  expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
  expect(panel('keyword').hasAttribute('inert')).toBe(true)
  expect(panel('conceptual').hasAttribute('inert')).toBe(false)
  expect(panel('conceptual').hidden).toBe(false)
  expect(document.getElementById(tab('conceptual').getAttribute('aria-controls')!)).toBe(panel('conceptual'))
  expect(container.querySelector('input[name="search-mode"]')).toBeNull()
  expect(container.querySelector('#search-input-help')!.textContent).toBe('search.input.helpConceptual')
  await click(container.querySelector('.search-btn')!)
  expect(value.onSubmitSearch).toHaveBeenCalledOnce()
  expect(value.onModeChange).not.toHaveBeenCalled()
})

it('switches by keyboard without submitting or resetting the shared query and keeps async updates in their channel', async () => {
  const value = props()
  await render({
    ...value,
    conceptualState: { phase: 'loading' },
  })
  expect(tab('conceptual').textContent).not.toContain('search.results.count')
  await act(() => tab('keyword').focus())
  await act(() => tab('keyword').dispatchEvent(new KeyboardEvent('keydown', {
    key: 'ArrowRight',
    bubbles: true,
  })))
  expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
  expect(document.activeElement).toBe(tab('conceptual'))
  expect(panel('conceptual').textContent).toContain('search.conceptual.loading')
  await render(value)
  expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
  expect(container.querySelector<HTMLInputElement>('.search-input')!.value).toBe(value.query)
  await click(tab('keyword'))
  await render({
    ...value,
    conceptualState: {
      phase: 'ready',
      response: {
        ...response,
        results: [],
      },
    },
  })
  expect(tab('keyword').getAttribute('aria-selected')).toBe('true')
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  expect(value.onChangeQuery).not.toHaveBeenCalled()
})

it('reveals ten returned cards at a time per channel, retains expansion and keeps source opening unchanged', async () => {
  const value = props()
  await render(value)
  expect(tab('keyword').textContent).toContain('"count":23')
  expect(tab('conceptual').textContent).toContain('"count":23')
  expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(10)
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(10)
  await click(panel('keyword').querySelector('summary')!)
  const details = panel('keyword').querySelector('details')!
  await click(more('keyword'))
  expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(20)
  await click(tab('conceptual'))
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(10)
  await click(panel('conceptual').querySelector('summary')!)
  const context = panel('conceptual').querySelector('details')!
  await click(context.querySelector('.result-evidence-action')!)
  const blocks = context.querySelectorAll('.conceptual-context-block').length
  await click(more('conceptual'))
  await act(() => more('conceptual').focus())
  await click(more('conceptual'))
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(23)
  expect(more('conceptual').getAttribute('aria-disabled')).toBe('true')
  expect(document.activeElement).toBe(more('conceptual'))
  await click(more('conceptual'))
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(23)
  expect(panel('conceptual').querySelector('.result-list-more')!.textContent).toContain('"shown":23,"total":23')
  await click(tab('keyword'))
  expect(details.open).toBe(true)
  expect(panel('keyword').querySelector('details')).toBe(details)
  expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(20)
  await click(panel('keyword').querySelector('.result-card-primary')!)
  expect(value.onViewResult).toHaveBeenCalledWith(value.results[0], {
    text: 'connections',
    sectionIndex: 2,
    pageIndex: 2,
  })
  await click(tab('conceptual'))
  expect(context.open).toBe(true)
  expect(context.querySelectorAll('.conceptual-context-block')).toHaveLength(blocks)
  await click(panel('conceptual').querySelector('.result-card-primary')!)
  expect(value.onViewConceptualResult).toHaveBeenCalledWith(response.results[0].url, {
    text: response.results[0].text,
    sectionIndex: 2,
    pageIndex: 2,
  })
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
})

it('resets list windows for a new request/scope but retains the chosen view and counts only scoped returned documents', async () => {
  const value = props()
  await render(value)
  await click(more('keyword'))
  await click(tab('conceptual'))
  await click(more('conceptual'))
  await render({
    ...value,
    searchRequest: {
      revision: 2,
      scope: 'selected',
    },
    scopeActive: true,
    scopeUrls: new Set(value.results.slice(4, 16).map(result => result.url)),
    conceptualState: {
      phase: 'ready',
      response: {
        ...response,
        results: response.results.slice(4, 16),
      },
    },
  })
  expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
  expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(10)
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(10)
  expect(tab('keyword').textContent).toContain('"count":12')
  expect(tab('conceptual').textContent).toContain('"count":12')
  expect(panel('keyword').textContent).not.toContain('Keyword 0')
  expect(panel('keyword').querySelector('.result-list-more')!.textContent).toContain('"shown":10,"total":12')
})

it.each(['partial', 'missing-index', 'incompatible-index', 'unavailable', 'preparing', 'failed'] as const)(
  'retains selectable %s fallback and never reports it as zero conceptual results', async reason => {
    const value = props()
    await render({
      ...value,
      conceptualState: {
        phase: 'fallback',
        reason,
        response: {
          ...response,
          availability: 'partial',
        },
      },
    })
    await click(tab('conceptual'))
    expect(panel('conceptual').textContent).toContain(`search.conceptual.${reason}`)
    expect(panel('conceptual').querySelector('.result-card')).toBeNull()
    expect(tab('conceptual').textContent).not.toContain('search.results.count')
    await click(tab('keyword'))
    expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(10)
  },
)

it('restores each list scroll position after a scrolled switch and forgets positions for a new query', async () => {
  const value = props()
  await render(value)
  const area = container.querySelector('.search-result-views')!
  vi.spyOn(area, 'getBoundingClientRect').mockReturnValue({ top: -700 } as DOMRect)
  vi.spyOn(window, 'scrollY', 'get').mockReturnValue(1000)
  await click(tab('conceptual'))
  expect(window.scrollTo).toHaveBeenLastCalledWith({
    top: 300,
    behavior: 'instant',
  })
  vi.spyOn(window, 'scrollY', 'get').mockReturnValue(500)
  await click(tab('keyword'))
  expect(window.scrollTo).toHaveBeenLastCalledWith({
    top: 1000,
    behavior: 'instant',
  })
  await render({
    ...value,
    searchRequest: {
      revision: 2,
      scope: '*',
    },
  })
  await click(tab('conceptual'))
  expect(window.scrollTo).toHaveBeenLastCalledWith({
    top: 0,
    behavior: 'instant',
  })
})

it('restores lists relative to the nonzero sticky inset and leaves unscrolled controls alone', async () => {
  const value = props()
  await render(value)
  const area = container.querySelector('.search-result-views')!
  const toolbar = container.querySelector<HTMLElement>('.search-options-toolbar')!
  const css = readFileSync('src/components/SearchBar/SearchBar.css', 'utf8')
  const stickyRules = css.match(/\.search-options-toolbar-sticky\s*\{[^}]*\}/g)!
  expect(stickyRules[0]).toContain('top: env(safe-area-inset-top, 0px)')
  expect(stickyRules[1]).toContain('top: var(--mobile-safe-top-space, env(safe-area-inset-top, 0px))')
  // jsdom does not resolve env()/media geometry; supply the browser's resolved inset.
  toolbar.style.top = '24px'
  const rect = vi.spyOn(area, 'getBoundingClientRect').mockReturnValue({ top: 10 } as DOMRect)
  vi.spyOn(window, 'scrollY', 'get').mockReturnValue(1000)
  await click(tab('conceptual'))
  expect(window.scrollTo).toHaveBeenLastCalledWith({
    top: 986,
    behavior: 'instant',
  })
  vi.spyOn(window, 'scrollY', 'get').mockReturnValue(500)
  await click(tab('keyword'))
  expect(window.scrollTo).toHaveBeenLastCalledWith({
    top: 1000,
    behavior: 'instant',
  })
  vi.mocked(window.scrollTo).mockClear()
  rect.mockReturnValue({ top: 80 } as DOMRect)
  await click(tab('conceptual'))
  expect(window.scrollTo).not.toHaveBeenCalled()
  expect(container.querySelector<HTMLInputElement>('.search-input')!.value).toBe(value.query)
})

it('distinguishes pending/failed keyword counts from ready empty results and keeps reader busy guards in both views', async () => {
  const value = props()
  await render({
    ...value,
    loading: true,
  })
  expect(tab('keyword').textContent).not.toContain('search.results.count')
  await click(tab('conceptual'))
  await render({
    ...value,
    searchFailed: true,
    results: [],
    openingDisabled: true,
  })
  expect(tab('keyword').textContent).not.toContain('search.results.count')
  await click(panel('conceptual').querySelector('.result-card-primary')!)
  expect(value.onViewConceptualResult).not.toHaveBeenCalled()
  await click(tab('keyword'))
  expect(panel('keyword').querySelector('[role="alert"]')!.textContent).toContain('search.results.searchFailed')
  await render({
    ...value,
    results: [],
    conceptualState: {
      phase: 'ready',
      response: {
        ...response,
        results: [],
      },
    },
  })
  expect(tab('keyword').textContent).toContain('"count":0')
  expect(tab('conceptual').textContent).toContain('"count":0')
  await render({
    ...value,
    openingDisabled: true,
  })
  await click(panel('keyword').querySelector('.result-card-primary')!)
  expect(value.onViewResult).not.toHaveBeenCalled()
})

it('groups the selector and keyword options before the shared scope while preserving matching callbacks and input descriptions', async () => {
  const value = props()
  await render(value)
  const input = container.querySelector<HTMLInputElement>('.search-input')!
  const toolbar = container.querySelector('.search-options-toolbar')!
  const helper = container.querySelector('#search-input-help')!
  const scope = container.querySelector('.search-scope')!
  expect(toolbar.contains(tab('keyword'))).toBe(true)
  expect(toolbar.contains(container.querySelector('.search-mode'))).toBe(true)
  expect(toolbar.contains(helper)).toBe(false)
  expect(toolbar.contains(scope)).toBe(false)
  expect(input.compareDocumentPosition(toolbar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  expect(toolbar.compareDocumentPosition(helper) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  expect(helper.compareDocumentPosition(scope) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  expect(scope.compareDocumentPosition(panel('keyword')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  await click(container.querySelectorAll<HTMLInputElement>('input[name="search-mode"]')[1])
  expect(value.onModeChange).toHaveBeenCalledWith('broader')
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  await render({
    ...value,
    mode: 'broader',
    queryError: 'tooLong',
  })
  expect(container.querySelector('#search-input-help')!.textContent).toBe('search.input.helpBroader')
  for (const id of input.getAttribute('aria-describedby')!.split(' ')) {
    expect(document.getElementById(id)).not.toBeNull()
  }
  await click(scope.querySelector<HTMLElement>('.panel-toggle')!)
  await click(tab('conceptual'))
  expect(container.querySelector('.search-scope')).toBe(scope)
  expect(scope.querySelector('.panel-toggle')!.getAttribute('aria-expanded')).toBe('true')
  expect(toolbar.contains(tab('conceptual'))).toBe(true)
  expect(toolbar.querySelector('input[name="search-mode"]')).toBeNull()
  expect(document.getElementById('search-input-help')!.textContent).toBe('search.input.helpConceptual')
  expect(input.value).toBe(value.query)
  await click(tab('keyword'))
  expect(container.querySelectorAll<HTMLInputElement>('input[name="search-mode"]')[1].checked).toBe(true)
})

it('keeps examples in empty Keyword view, hides them in Conceptual and preserves explicit query/submission behavior', async () => {
  const value = {
    ...props(),
    query: '',
    submittedQuery: '',
    results: [],
    conceptualState: null,
  }
  await render(value)
  const examples = container.querySelectorAll<HTMLButtonElement>('.search-example')
  expect(examples).toHaveLength(2)
  await click(examples[0])
  expect(value.onChangeQuery).toHaveBeenCalledWith('search.input.exampleBroad')
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  await click(tab('conceptual'))
  expect(container.querySelector('.search-examples')).toBeNull()
  expect(container.querySelector<HTMLInputElement>('.search-input')!.value).toBe('')
  await click(tab('keyword'))
  expect(container.querySelectorAll('.search-example')).toHaveLength(2)
  await render({
    ...value,
    disabled: true,
  })
  await click(container.querySelector('.search-example')!)
  expect(value.onChangeQuery).toHaveBeenCalledTimes(1)
  await render({
    ...value,
    query: 'connections',
  })
  expect(container.querySelector('.search-examples')).toBeNull()
  await act(() => container.querySelector('.search-input')!.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Enter',
    bubbles: true,
  })))
  expect(value.onSubmitSearch).toHaveBeenCalledOnce()
})

it('keeps mode-specific pre-submit prompts and replaces them on submission without clearing the draft', async () => {
  const value = {
    ...props(),
    query: 'draft connections',
    submittedQuery: '',
    conceptualState: null,
    lastSearchInfo: null,
  }
  await render(value)
  expect(panel('keyword').querySelector('.welcome > p')?.textContent).toBe('search.results.welcome')
  expect(panel('keyword').querySelector('.welcome-hint')?.textContent).toBe('search.results.welcomeHint')
  await click(tab('conceptual'))
  expect(panel('conceptual').querySelector('.welcome')?.textContent).toBe('search.conceptual.welcome')
  expect(container.querySelector<HTMLInputElement>('.search-input')!.value).toBe(value.query)
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  await render({
    ...value,
    submittedQuery: value.query,
    conceptualState: { phase: 'loading' },
  })
  expect(panel('conceptual').querySelector('.welcome')).toBeNull()
  expect(panel('conceptual').textContent).toContain('search.conceptual.loading')
  await render({
    ...value,
    submittedQuery: value.query,
    conceptualState: {
      phase: 'ready',
      response,
    },
  })
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(10)
  expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
})

it('transitions setup guidance without claiming missing indexes during loading, errors or an empty library', async () => {
  const value = {
    ...props(),
    query: 'unsubmitted query',
    submittedQuery: '',
    conceptualState: null,
    scopeActive: true,
    scopeUrls: new Set(['/uploads/guide-1.pdf']),
    selectedFilters: new Set(['/uploads/guide-1.pdf']),
  }
  await render({
    ...value,
    semanticStatus: 'needsIndex',
    hasStoredIndex: false,
  })
  await click(tab('conceptual'))
  const warning = panel('conceptual').querySelector('.conceptual-index-warning')!
  expect(warning.textContent).toBe('settings.semantic.needsIndex')
  expect(panel('conceptual').querySelectorAll('.welcome p')).toHaveLength(1)
  expect(warning.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true')
  const open = panel('conceptual').querySelector<HTMLButtonElement>('[aria-haspopup="dialog"]')!
  expect(open.firstElementChild?.tagName.toLowerCase()).toBe('svg')
  expect(open.firstElementChild?.getAttribute('aria-hidden')).toBe('true')
  await act(() => open.focus())
  await click(open)
  expect(value.onOpenSearchSettings).toHaveBeenCalledOnce()
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  await render({
    ...value,
    semanticStatus: 'needsIndex',
    hasStoredIndex: true,
  })
  expect(panel('conceptual').querySelector('.conceptual-index-warning')?.textContent).toBe('search.conceptual.indexUpdateRequired')
  for (const status of ['checking', 'loading', 'downloading', 'indexing', 'cancelling', 'disabling'] as const) {
    await render({
      ...value,
      semanticStatus: status,
    })
    expect(panel('conceptual').querySelector('.conceptual-index-warning')).toBeNull()
    expect(panel('conceptual').querySelector('.spinner')?.getAttribute('aria-hidden')).toBe('true')
    expect(panel('conceptual').querySelector('[aria-haspopup="dialog"]')).toBeNull()
  }
  for (const status of ['statusUnavailable', 'fallback', 'empty', 'ready'] as const) {
    await render({
      ...value,
      semanticStatus: status,
    })
    expect(panel('conceptual').querySelector('.conceptual-index-warning')).toBeNull()
    expect(panel('conceptual').querySelector('.spinner')).toBeNull()
  }
  expect(panel('conceptual').textContent).toBe('search.conceptual.welcome')
  expect(container.querySelector<HTMLInputElement>('.search-input')!.value).toBe(value.query)
  expect(value.onClearFilters).not.toHaveBeenCalled()
  await click(tab('keyword'))
  expect(panel('keyword').querySelector('.welcome > p')?.textContent).toBe('search.results.welcome')
  expect(panel('keyword').querySelector('.welcome-hint')?.textContent).toBe('search.results.welcomeHint')
})

it('uses scoped query coverage over idle global warnings and opens Settings without resetting either channel', async () => {
  const value = {
    ...props(),
    semanticStatus: 'needsIndex' as const,
    scopeActive: true,
    scopeUrls: new Set(['/uploads/guide-1.pdf']),
  }
  await render(value)
  await click(tab('conceptual'))
  expect(panel('conceptual').querySelector('.conceptual-index-warning')).toBeNull()
  expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(10)
  for (const reason of ['missing-index', 'incompatible-index', 'partial'] as const) {
    await render({
      ...value,
      conceptualState: {
        phase: 'fallback',
        reason,
        response: {
          ...response,
          availability: reason,
          indexedDocuments: 1,
          eligibleDocuments: 2,
          results: [],
        },
      },
    })
    expect(panel('conceptual').querySelectorAll('.result-card')).toHaveLength(0)
    expect(panel('conceptual').querySelector('.conceptual-index-warning')?.textContent).toBe(`search.conceptual.${reason}`)
    await click(panel('conceptual').querySelector('[aria-haspopup="dialog"]')!)
    expect(tab('conceptual').getAttribute('aria-selected')).toBe('true')
  }
  expect(value.onOpenSearchSettings).toHaveBeenCalledTimes(3)
  expect(value.onSubmitSearch).not.toHaveBeenCalled()
  expect(value.onChangeQuery).not.toHaveBeenCalled()
  expect(value.onClearFilters).not.toHaveBeenCalled()
  await click(tab('keyword'))
  expect(panel('keyword').querySelectorAll('.result-card')).toHaveLength(1)
})

it('leaves validation feedback in charge rather than showing setup or no-results text for an invalid query', async () => {
  const value = {
    ...props(),
    submittedQuery: '',
    conceptualState: null,
    queryError: 'unmatchedQuote' as const,
  }
  await render(value)
  await click(tab('conceptual'))
  expect(panel('conceptual').querySelector('.welcome')).toBeNull()
  expect(panel('conceptual').textContent).not.toContain('search.conceptual.empty')
  expect(container.textContent).toContain('search.input.unmatchedQuote')
  expect(value.onOpenSearchSettings).not.toHaveBeenCalled()
})
