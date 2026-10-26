import {
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import {
  invoke,
  isTauri,
} from '@tauri-apps/api/core'
import type { DocumentInfo } from '../types/search'
import type { SearchQueryError } from '../hooks/useSearch'
import { parseSearchQuery } from '../utils/phraseSearch'
import type { SemanticSearchController } from './useSemanticSearch'

export interface SourceContext {
  sectionIndex: number
  pageIndex: number | null
  sectionTitle: string | null
  byteStart: number
  byteEnd: number
  text: string
}
export interface ConceptualResult extends SourceContext {
  documentId: string
  url: string
  title: string
  format: string
  score: number
  provenance: 'conceptual'
  context: SourceContext[]
}
export interface ConceptualResponse {
  availability: 'missing-index' | 'incompatible-index' | 'partial' | 'ready'
  results: ConceptualResult[]
  indexedDocuments: number
  eligibleDocuments: number
  scannedPassages: number
}
export type ConceptualState = {
  phase: 'loading' | 'ready' | 'fallback'
  reason?: 'unavailable' | 'preparing' | 'missing-index' | 'incompatible-index' | 'partial' | 'failed'
  response?: ConceptualResponse
} | null

/** Independent opt-in channel: never delays lexical results or reinterprets their counts.
 * Request identity hides old results immediately; cleanup rejects late IPC completion. */
export function useConceptualSearch({ semantic, submittedQuery, searchRequest, queryError,
  scopeUrls, scopeActive, library }: {
  semantic: SemanticSearchController
  submittedQuery: string
  searchRequest: { revision: number;
    scope: string }
  queryError: SearchQueryError | null
  scopeUrls: Set<string>
  scopeActive: boolean
  library: DocumentInfo[]
}): ConceptualState {
  const scopeKey = scopeActive ? JSON.stringify([...scopeUrls].sort()) : ''
  const { enabled, format, provider, busy, index } = semantic
  // Index/library snapshots invalidate results after explicit jobs and source mutations.
  const key = useMemo(() => ({
    searchRequest,
    library,
    index,
    parsed: parseSearchQuery(submittedQuery),
    enabled,
    queryError,
    busy,
    active: !!provider?.compiled && provider.activeFormat === format,
    urls: scopeKey ? JSON.parse(scopeKey) as string[] : undefined,
  }), [submittedQuery, searchRequest, queryError, scopeKey, library, enabled, format,
    provider?.compiled, provider?.activeFormat, busy, index])
  const [state, setState] = useState<{ key: typeof key;
    value: ConceptualState } | null>(null)
  const running = useRef(false)
  const latest = useRef<(() => void) | null>(null)
  // Filter changes hide old results immediately, then wait for the matching lexical submission.
  const eligible = key.enabled && searchRequest.scope === scopeKey && !key.queryError &&
    !key.parsed.unmatchedQuote && !!key.parsed.providerQuery

  useEffect(() => {
    if (!eligible) {
      return
    }
    if (!isTauri() || !key.active || key.busy) {
      return
    }
    let closed = false
    // Native queries share one slot; coalesce replacements until the running IPC settles.
    const run = () => {
      latest.current = null
      running.current = true
      // A microtask lets StrictMode retire its first effect before reserving a native query slot.
      void Promise.resolve().then(() => {
        if (closed) {
          return
        }
        return invoke<ConceptualResponse>('search_semantic_query', {
          request: {
            query: key.parsed.unquotedText,
            exactPhrases: key.parsed.exactPhrases,
            documentUrls: key.urls,
            limit: 50,
          },
        })
      }).then((response) => {
        if (closed || !response) {
          return
        }
        // Partial coverage uses lexical fallback rather than hiding unindexed library sources.
        setState({
          key,
          value: response.availability === 'ready'
            ? {
              phase: 'ready',
              response,
            }
            : {
              phase: 'fallback',
              reason: response.availability,
              response,
            },
        })
      }).catch(() => {
        if (!closed) {
          setState({
            key,
            value: {
              phase: 'fallback',
              reason: 'failed',
            },
          })
        }
      }).finally(() => {
        running.current = false
        latest.current?.()
      })
    }
    latest.current = run
    if (!running.current) {
      run()
    }
    return () => {
      closed = true
      if (latest.current === run) {
        latest.current = null
      }
    }
  }, [key, eligible])

  if (!eligible) {
    return null
  }
  if (!isTauri() || !key.active) {
    return {
      phase: 'fallback',
      reason: 'unavailable',
    }
  }
  if (key.busy) {
    return {
      phase: 'fallback',
      reason: 'preparing',
    }
  }
  return state?.key === key ? state.value : { phase: 'loading' }
}
