import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import type {
  TtsChunk,
  TtsChunkSourceSpan,
} from '../types'
import { graftImportedSourceSpans } from '../alignment/importedSourceSpans'
import { normalizeSegmentText } from '../alignment/readableSegments'
import { chunkReadableSegmentsWithSpans } from '../utils/text'
import { createChunkSourceResolver } from './sourceChunkResolver'

vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsDiagnostic: vi.fn() }))

function chunk(id: string, start: number, end: number, segmentIndex = 0): TtsChunk {
  return {
    id,
    text: 'Saved narration',
    sourceSpan: {
      startSegmentIndex: segmentIndex,
      startOffset: start,
      endSegmentIndex: segmentIndex,
      endOffset: end,
    },
  }
}

function ready(chunks: readonly TtsChunk[], lengths: readonly number[]) {
  const resolver = createChunkSourceResolver(chunks, lengths)
  expect(resolver.status).toBe('ready')
  if (resolver.status !== 'ready') {
    throw new Error(resolver.reason)
  }
  return resolver
}

describe('play-anywhere source chunk resolver', () => {
  it('chooses the containing canonical chunk at half-open boundaries', () => {
    const resolver = ready([chunk('first', 0, 5), chunk('second', 5, 10)], [12])
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 0,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: 'first',
      })
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 4,
    }).status).toBe('resolved')
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 5,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 1,
        chunkId: 'second',
      })
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 9,
    }).status).toBe('resolved')
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 10,
    }))
      .toEqual({
        status: 'unavailable',
        reason: 'unmapped',
      })
  })

  it('keeps playback indices when empty source chunks are filtered', () => {
    const chunks = [
      {
        id: 'empty',
        text: ' \n ',
      }, chunk('first', 0, 3),
      {
        id: 'also-empty',
        text: '',
      }, chunk('second', 4, 8),
    ]
    expect(ready(chunks, [8]).resolve({
      segmentIndex: 0,
      offset: 4,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 1,
        chunkId: 'second',
      })
  })

  it('maps merged chunks across segments and leaves source gaps unmapped', () => {
    const merged = chunk('merged', 2, 4)
    merged.sourceSpan!.endSegmentIndex = 2
    const resolver = ready([merged, chunk('next', 1, 5, 3)], [6, 9, 7, 6])
    for (const point of [
      {
        segmentIndex: 0,
        offset: 5,
      }, {
        segmentIndex: 1,
        offset: 8,
      },
      {
        segmentIndex: 2,
        offset: 3,
      },
    ]) {
      expect(resolver.resolve(point))
        .toEqual({
          status: 'resolved',
          chunkIndex: 0,
          chunkId: 'merged',
        })
    }
    for (const point of [
      {
        segmentIndex: 0,
        offset: 1,
      }, {
        segmentIndex: 2,
        offset: 4,
      },
      {
        segmentIndex: 3,
        offset: 0,
      }, {
        segmentIndex: 3,
        offset: 5,
      },
    ]) {
      expect(resolver.resolve(point)).toEqual({
        status: 'unavailable',
        reason: 'unmapped',
      })
    }
  })

  it('skips spanless chunks and keeps the accepted neighbours playable', () => {
    const imported = [{
      id: 'legacy',
      text: 'Saved narration',
      textHash: 'saved-hash',
    }]
    const before = JSON.stringify(imported)
    // Durable path: a spanless chunk is excluded from matching instead of
    // disabling the book, so a spanless-only list has nothing to play from.
    expect(createChunkSourceResolver(imported, [15]))
      .toEqual({
        status: 'unavailable',
        reason: 'not-ready',
      })
    const recovered = [{
      ...imported[0],
      sourceSpan: chunk('legacy', 0, 15).sourceSpan,
    }]
    expect(ready(recovered, [15]).resolve({
      segmentIndex: 0,
      offset: 7,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: 'legacy',
      })
    expect(JSON.stringify(imported)).toBe(before)
    expect(recovered[0].textHash).toBe(imported[0].textHash)
    const partial = ready([chunk('first', 0, 4), ...imported], [15])
    expect(partial.resolve({
      segmentIndex: 0,
      offset: 2,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: 'first',
      })
    // Points inside the skipped chunk stay unmapped: never guess audio.
    expect(partial.resolve({
      segmentIndex: 0,
      offset: 8,
    }))
      .toEqual({
        status: 'unavailable',
        reason: 'unmapped',
      })
  })

  it('returns not-ready for an empty canonical sequence', () => {
    expect(createChunkSourceResolver([], []))
      .toEqual({
        status: 'unavailable',
        reason: 'not-ready',
      })
    expect(createChunkSourceResolver([{
      id: 'blank',
      text: ' ',
    }], [4]))
      .toEqual({
        status: 'unavailable',
        reason: 'not-ready',
      })
  })

  it('accepts the existing legacy graft without rewriting saved narration or hashes', () => {
    const rebuilt = [
      {
        ...chunk('first', 0, 4),
        text: 'One.',
      },
      {
        ...chunk('second', 5, 9),
        text: 'One.',
      },
    ]
    const imported = rebuilt.map((value, index) => ({
      id: value.id,
      text: value.text,
      textHash: `saved-hash-${index}`,
    }))
    const before = JSON.stringify(imported)
    const recovered = graftImportedSourceSpans(imported, rebuilt)
    expect(recovered).not.toBeNull()
    expect(ready(recovered!, [9]).resolve({
      segmentIndex: 0,
      offset: 7,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 1,
        chunkId: 'second',
      })
    expect(recovered!.map(({ id, text, textHash }) => ({
      id,
      text,
      textHash,
    }))).toEqual(imported)
    expect(JSON.stringify(imported)).toBe(before)
    expect(graftImportedSourceSpans(imported, [chunk('different-id', 0, 8), rebuilt[1]]))
      .toBeNull()
    expect(graftImportedSourceSpans(imported, [{
      ...rebuilt[0],
      text: 'Changed narration',
    }, rebuilt[1]]))
      .toBeNull()
    expect(JSON.stringify(imported)).toBe(before)
  })

  it.each([
    {
      segmentIndex: -1,
      offset: 0,
    }, {
      segmentIndex: 0.5,
      offset: 0,
    },
    {
      segmentIndex: NaN,
      offset: 0,
    }, {
      segmentIndex: Infinity,
      offset: 0,
    },
    {
      segmentIndex: 1,
      offset: 0,
    }, {
      segmentIndex: 0,
      offset: -1,
    },
    {
      segmentIndex: 0,
      offset: 0.5,
    }, {
      segmentIndex: 0,
      offset: NaN,
    },
    {
      segmentIndex: 0,
      offset: Infinity,
    }, {
      segmentIndex: 0,
      offset: 5,
    },
    {
      segmentIndex: 0,
      offset: Number.MAX_SAFE_INTEGER + 1,
    },
  ])('rejects an invalid or end-of-document point: %j', (point) => {
    expect(ready([chunk('one', 0, 5)], [5]).resolve(point))
      .toEqual({
        status: 'unavailable',
        reason: 'invalid-point',
      })
  })

  it.each([
    { startSegmentIndex: -1 }, { startOffset: -1 }, { startOffset: 1.5 },
    { startOffset: NaN }, { startOffset: Infinity }, { startOffset: 5 },
    { endOffset: 0 }, { endOffset: 6 }, { endOffset: NaN },
    { endOffset: Infinity }, { endOffset: 2.5 }, { endSegmentIndex: 1 },
    { endSegmentIndex: -1 }, { endSegmentIndex: 0.5 },
    { endSegmentIndex: Infinity }, {
      startOffset: 3,
      endOffset: 2,
    },
    {
      startOffset: 2,
      endOffset: 2,
    },
  ] satisfies Partial<TtsChunkSourceSpan>[])('excludes chunks with invalid source bounds: %j', (patch) => {
    const source = chunk('one', 0, 5)
    source.sourceSpan = {
      ...source.sourceSpan!,
      ...patch,
    }
    expect(createChunkSourceResolver([source], [5]))
      .toEqual({
        status: 'unavailable',
        reason: 'not-ready',
      })
  })

  it.each([-1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])(
    'rejects invalid segment lengths: %s', (length) => {
      expect(createChunkSourceResolver([chunk('one', 0, 5)], [length]))
        .toEqual({
          status: 'unavailable',
          reason: 'invalid-source',
        })
    },
  )

  it('rejects unsorted, overlapping and duplicate source identities without sorting them', () => {
    expect(createChunkSourceResolver([chunk('later', 5, 8), chunk('earlier', 0, 3)], [8]))
      .toEqual({
        status: 'unavailable',
        reason: 'invalid-source',
      })
    expect(createChunkSourceResolver([chunk('one', 0, 5), chunk('two', 4, 8)], [8]))
      .toEqual({
        status: 'unavailable',
        reason: 'ambiguous',
      })
    expect(createChunkSourceResolver([chunk('one', 0, 5), chunk('two', 0, 5)], [5]))
      .toEqual({
        status: 'unavailable',
        reason: 'ambiguous',
      })
    expect(createChunkSourceResolver([chunk('one', 0, 4), chunk('one', 4, 8)], [8]))
      .toEqual({
        status: 'unavailable',
        reason: 'invalid-source',
      })
    expect(createChunkSourceResolver([chunk('', 0, 5)], [5]))
      .toEqual({
        status: 'unavailable',
        reason: 'invalid-source',
      })
    // Duplicate identity stays a global defect even on an otherwise skippable chunk.
    expect(createChunkSourceResolver([chunk('one', 0, 4), {
      ...chunk('one', 4, 8),
      sourceSpan: undefined,
    }], [8]))
      .toEqual({
        status: 'unavailable',
        reason: 'invalid-source',
      })
  })

  it('snapshots indices without retaining mutable source spans or lengths', () => {
    const chunks = [chunk('one', 0, 5)]
    const lengths = [5]
    const before = JSON.stringify(chunks)
    const resolver = ready(chunks, lengths)
    expect(JSON.stringify(chunks)).toBe(before)
    chunks[0].sourceSpan!.endOffset = 1
    chunks[0].id = 'replacement'
    lengths[0] = 1
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 4,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: 'one',
      })
  })

  it('uses source offsets to distinguish repeated text, Arabic marks and UTF-16 emoji', () => {
    const texts = ['Echo. Echo.', 'بِدايَةٌ. نِهايَةٌ.', 'A😀B. C😀D.']
    const chunks = texts.flatMap((text, segmentIndex) => {
      const split = text.indexOf(' ') + 1
      return [chunk(`${segmentIndex}-first`, 0, split - 1, segmentIndex),
        chunk(`${segmentIndex}-second`, split, text.length, segmentIndex)]
    })
    const resolver = ready(chunks, texts.map((text) => text.length))
    texts.forEach((text, segmentIndex) => {
      const offset = text.indexOf(' ') + 1
      expect(resolver.resolve({
        segmentIndex,
        offset,
      }))
        .toEqual({
          status: 'resolved',
          chunkIndex: segmentIndex * 2 + 1,
          chunkId: `${segmentIndex}-second`,
        })
    })
    expect(resolver.resolve({
      segmentIndex: 2,
      offset: 2,
    }))
      .toEqual({
        status: 'resolved',
        chunkIndex: 4,
        chunkId: '2-first',
      })
  })

  it('matches a linear oracle at every position in a production-chunked synthetic book', () => {
    const segments = Array.from({ length: 40 }, (_, index) => ({
      kind: 'paragraph' as const,
      text: index % 2 ? 'First echo. First echo. Last sentence.' : 'بِدايَةٌ واضحةٌ. نِهايَةٌ واضحةٌ.',
    }))
    const chunks = chunkReadableSegmentsWithSpans(segments, {
      maxChunkLength: 24,
      minChunkLength: 8,
    })
      .map((value, index) => ({
        ...value,
        id: `saved-${index}`,
      }))
    const before = JSON.stringify(chunks)
    const texts = segments.map((segment) => normalizeSegmentText(segment.text))
    const resolver = ready(chunks, texts.map((text) => text.length))
    texts.forEach((text, segmentIndex) => {
      for (let offset = 0; offset < text.length; offset++) {
        const index = chunks.findIndex(({ sourceSpan: span }) => span &&
          (segmentIndex > span.startSegmentIndex ||
            (segmentIndex === span.startSegmentIndex && offset >= span.startOffset)) &&
          (segmentIndex < span.endSegmentIndex ||
            (segmentIndex === span.endSegmentIndex && offset < span.endOffset)))
        expect(resolver.resolve({
          segmentIndex,
          offset,
        })).toEqual(index < 0
          ? {
            status: 'unavailable',
            reason: 'unmapped',
          }
          : {
            status: 'resolved',
            chunkIndex: index,
            chunkId: chunks[index].id,
          })
      }
    })
    expect(JSON.stringify(chunks)).toBe(before)
  })
})
