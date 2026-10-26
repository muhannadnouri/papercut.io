// @vitest-environment jsdom
import {
  act,
  useRef,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { useTtsHighlight } from './useTtsHighlight'
import {
  highlightTtsChunk,
  scrollRangeIntoView,
} from '../alignment/highlightRanges'
vi.mock('../alignment/highlightRanges', () => ({
  clearTtsHighlight: vi.fn(),
  clearTtsWordHighlight: vi.fn(),
  getOrBuildSegmentIndex: vi.fn(),
  highlightTtsChunk: vi.fn(),
  highlightTtsWord: vi.fn(),
  invalidateTtsDomCaches: vi.fn(),
  scrollRangeIntoView: vi.fn(),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => {
  vi.useRealTimers(); vi.unstubAllGlobals(); vi.clearAllMocks(); document.body.replaceChildren()
})
it('updates highlights while browsing, cancels delayed reveal immediately and reveals the same paused chunk on Return', async () => {
  vi.useFakeTimers(); vi.stubGlobal('requestAnimationFrame', (fn: FrameRequestCallback) => window.setTimeout(() => fn(0), 0)); vi.stubGlobal('cancelAnimationFrame', window.clearTimeout)
  const range = document.createRange(); vi.mocked(highlightTtsChunk).mockReturnValue({ range } as ReturnType<typeof highlightTtsChunk>)
  let follow = true
  const policy = {
    allowsReveal: () => follow,
    revision: 0,
  }; const chunks = [{
    id: 'a',
    text: 'A',
  }, {
    id: 'b',
    text: 'B',
  }]
  function Harness({ index = 0, revision = 0 }) {
    const root = useRef<HTMLElement | null>(null); useTtsHighlight(root, {
      enabled: true,
      currentChunkIndex: index,
      chunks,
      wordHighlightEnabled: false,
      audioFollowPolicy: {
        ...policy,
        revision,
      },
    }); return <article ref={root}>A B</article>
  }
  const element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  try {
    await act(async () => app.render(<Harness />)); await act(async () => vi.advanceTimersByTime(1))
    follow = false; await act(async () => vi.advanceTimersByTime(150))
    expect(highlightTtsChunk).toHaveBeenCalled(); expect(scrollRangeIntoView).not.toHaveBeenCalled()
    await act(async () => app.render(<Harness index={1} />)); await act(async () => vi.advanceTimersByTime(150))
    expect(vi.mocked(highlightTtsChunk).mock.calls.at(-1)![1]).toBe(1); expect(scrollRangeIntoView).not.toHaveBeenCalled()
    follow = true; await act(async () => app.render(<Harness index={1} revision={1} />)); await act(async () => vi.advanceTimersByTime(150))
    expect(scrollRangeIntoView).toHaveBeenCalledOnce()
  } finally {
    await act(async () => app.unmount())
  }
})
