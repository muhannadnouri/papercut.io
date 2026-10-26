// @vitest-environment jsdom
import {
  act,
  StrictMode,
  useLayoutEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { usePreparedTextTarget } from './usePreparedTextTarget'
import type { ViewerTextTargetResult } from '../../viewers/types'
import type { TtsChunk } from '../types'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())

it('invalidates pending and resolved targets on committed document, model and chunk replacement or unmount', async () => {
  let resolve!: ReturnType<typeof usePreparedTextTarget>
  const chunks: TtsChunk[] = [{
    id: 'a',
    text: 'First.',
    sourceSpan: {
      startSegmentIndex: 0,
      endSegmentIndex: 0,
      startOffset: 0,
      endOffset: 6,
    },
  }]
  const capture: ViewerTextTargetResult = {
    status: 'captured',
    point: {
      format: 'html',
      segmentIndex: 0,
      offset: 1,
    },
    segmentTexts: Object.freeze(['First.']),
    segmentLengths: [6],
    isCurrent: () => true,
  }
  const base: Parameters<typeof usePreparedTextTarget>[0] = {
    documentUrl: '/book.html',
    modelId: 'model-a',
    docContent: '<p>First.</p>',
    chunks,
    getChunks: async () => chunks,
    getPdfSegments: async () => [],
  }
  function Harness(props: typeof base) {
    const result = usePreparedTextTarget(props)
    useLayoutEffect(() => {
      resolve = result
    }, [result])
    return null
  }
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  let mounted = true
  const render = async (props: typeof base) => act(async () => app.render(<StrictMode><Harness {...props} /></StrictMode>))
  try {
    await render(base)
    let first = await resolve('/book.html', capture)
    expect(first.status).toBe('resolved')
    for (const props of [{
      ...base,
      modelId: 'model-b',
    }, {
      ...base,
      chunks: [...chunks],
    }, {
      ...base,
      documentUrl: '/other.html',
    }]) {
      await render(props)
      if (first.status === 'resolved') {
        expect(first.isCurrent()).toBe(false)
      }
      first = await resolve(props.documentUrl!, capture)
      expect(first.status).toBe('resolved')
    }
    let finish!: (chunks: TtsChunk[]) => void
    await render({
      ...base,
      chunks: null,
      getChunks: vi.fn(() => new Promise<TtsChunk[]>(yes => {
        finish = yes
      })),
    })
    const pending = resolve('/book.html', capture)
    await render({
      ...base,
      docContent: '<p>Other.</p>',
    })
    finish(chunks)
    expect(await pending).toMatchObject({ reason: 'stale' })
    await act(async () => app.unmount())
    mounted = false
    expect(await resolve('/book.html', capture)).toMatchObject({ reason: 'not-ready' })
  } finally {
    if (mounted) {
      await act(async () => app.unmount())
    }
  }
})
