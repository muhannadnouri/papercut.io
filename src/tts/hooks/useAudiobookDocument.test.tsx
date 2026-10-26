// @vitest-environment jsdom
import {
  act,
  useLayoutEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { useAudiobookDocument } from './useAudiobookDocument'
import { getImportedAudiobookMetadata } from '../api/nativeTts'
import { getUploadedPdfNarrationSegments } from '../../uploads/DocumentUploads'

vi.mock('../api/nativeTts', () => ({ getImportedAudiobookMetadata: vi.fn() }))
vi.mock('../../uploads/DocumentUploads', async importOriginal => ({
  ...await importOriginal<typeof import('../../uploads/DocumentUploads')>(),
  getUploadedPdfNarrationSegments: vi.fn(),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => {
  vi.clearAllMocks(); document.body.replaceChildren()
})

type Options = Parameters<typeof useAudiobookDocument>[0]
const base: Options = {
  selectedDoc: '/a.html',
  docContent: '<p>First.</p>',
  modelId: 'kokoro',
  loadHtmlDocument: async () => '<p>First.</p>',
  onImportedMetadata: () => {},
}

it('never exposes the previous document/model chunks during replacement and retains PDF runtime source spans before playback', async () => {
  let latest!: ReturnType<typeof useAudiobookDocument>
  const seen: Array<ReturnType<typeof useAudiobookDocument>['chunks']> = []
  function Harness(props: Options) {
    const result = useAudiobookDocument(props)
    useLayoutEffect(() => {
      latest = result; seen.push(result.chunks)
    }, [result])
    return null
  }
  const element = document.createElement('div'); document.body.append(element)
  const app = createRoot(element)
  try {
    await act(async () => app.render(<Harness {...base} />))
    expect(latest.canonicalChunks?.[0].text).toBe('First.')
    seen.length = 0
    let release!: (value: Awaited<ReturnType<typeof getUploadedPdfNarrationSegments>>) => void
    vi.mocked(getUploadedPdfNarrationSegments).mockReturnValue(new Promise(yes => {
      release = yes
    }))
    await act(async () => app.render(<Harness {...base} selectedDoc="/uploads/ab.pdf" docContent="" />))
    expect(seen.every(chunks => chunks === null)).toBe(true)
    await act(async () => release([{
      text: 'PDF.',
      sourceRuns: [{
        pageIndex: 0,
        blockOrder: 2,
        startOffset: 0,
        endOffset: 4,
        sourceStartOffset: 0,
        sourceEndOffset: 4,
      }],
    }]))
    expect(latest.canonicalChunks?.[0].pdfSourceSpans).toEqual([{
      pageIndex: 0,
      blockOrder: 2,
      startOffset: 0,
      endOffset: 4,
    }])
    seen.length = 0
    await act(async () => app.render(<Harness {...base} modelId="silma" />))
    expect(seen[0]).toBeNull()
    expect(latest.canonicalChunks?.[0].text).toBe('First.')
  } finally {
    await act(async () => app.unmount())
  }
})

it('excludes display fallback from canonical imported chunks and retries authoritative metadata rather than re-chunking', async () => {
  let latest!: ReturnType<typeof useAudiobookDocument>
  function Harness() {
    const result = useAudiobookDocument({
      ...base,
      selectedDoc: '/user-uploads/ab.html',
    })
    useLayoutEffect(() => {
      latest = result
    }, [result])
    return null
  }
  vi.mocked(getImportedAudiobookMetadata).mockRejectedValue(new Error('manifest unavailable'))
  const element = document.createElement('div'); document.body.append(element)
  const app = createRoot(element)
  try {
    await act(async () => app.render(<Harness />))
    expect(latest.chunks?.length).toBeGreaterThan(0)
    expect(latest.canonicalChunks).toBeNull()
    await expect(latest.getCanonicalChunks()).rejects.toThrow('manifest unavailable')
    expect(getImportedAudiobookMetadata).toHaveBeenCalledTimes(2)
  } finally {
    await act(async () => app.unmount())
  }
})
