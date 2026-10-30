import type { TtsChunk } from '../types'

export interface SourceTextPoint {
  segmentIndex: number
  offset: number
}

interface UnavailableTarget {
  status: 'unavailable'
  reason: 'not-ready' | 'unmapped' | 'ambiguous' | 'invalid-source' | 'invalid-point'
}

export type ChunkTargetResult =
  | { status: 'resolved';
    chunkIndex: number;
    chunkId: string }
  | UnavailableTarget

interface ChunkSourceEntry {
  start: SourceTextPoint
  end: SourceTextPoint
  chunkIndex: number
  chunkId: string
}

type ChunkSourceResolver =
  | { status: 'ready';
    resolve: (point: SourceTextPoint) => ChunkTargetResult
    spanAt: (chunkIndex: number) => { start: SourceTextPoint;
      end: SourceTextPoint } | null }
  | UnavailableTarget

/**
 * Where `point` sits inside the `start..end` span as a 0..1 character
 * fraction, or null when the point lies outside the span or the span is
 * empty. Points are compared over cumulative segment lengths.
 */
export function withinSpanFraction(
  point: SourceTextPoint,
  start: SourceTextPoint,
  end: SourceTextPoint,
  lengths: readonly number[],
): number | null {
  // Chunk spans use half-open ends, so the end offset may equal its segment
  // length; only the start needs the strict inside-the-segment check.
  if (!isTextPoint(start, lengths)) {
    return null
  }
  const endValid = Number.isSafeInteger(end.segmentIndex) && end.segmentIndex >= 0 &&
    Number.isSafeInteger(end.offset) && end.offset > 0 &&
    end.offset <= (lengths[end.segmentIndex] ?? -1)
  if (!endValid) {
    return null
  }
  const globalOffset = (value: SourceTextPoint) => {
    let total = 0
    for (let index = 0; index < value.segmentIndex && index < lengths.length; index++) {
      total += lengths[index]
    }
    return total + value.offset
  }
  const spanStart = globalOffset(start)
  const spanLength = globalOffset(end) - spanStart
  if (spanLength <= 0) {
    return null
  }
  const fraction = (globalOffset(point) - spanStart) / spanLength
  if (!Number.isFinite(fraction) || fraction < 0 || fraction >= 1) {
    return null
  }
  return fraction
}

/** Index validated/recovered spans once; points and lengths use normalized UTF-16. */
export function createChunkSourceResolver(
  chunks: readonly TtsChunk[],
  segmentLengths: readonly number[],
): ChunkSourceResolver {
  const lengths = [...segmentLengths]
  if (lengths.some((length) => !Number.isSafeInteger(length) || length < 0)) {
    return {
      status: 'unavailable',
      reason: 'invalid-source',
    }
  }

  const entries: ChunkSourceEntry[] = []
  const ids = new Set<string>()
  for (const chunk of chunks) {
    if (!chunk.text.trim()) {
      continue
    }
    if (!chunk.id.trim() || ids.has(chunk.id)) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      }
    }
    const span = chunk.sourceSpan
    // Degraded books keep playing: a chunk with no usable span is excluded
    // from play-from-here matching instead of disabling the whole book.
    // Structural defects (identity, ordering, overlap) still fail globally.
    if (!span) {
      continue
    }

    const start = {
      segmentIndex: span.startSegmentIndex,
      offset: span.startOffset,
    }
    const end = {
      segmentIndex: span.endSegmentIndex,
      offset: span.endOffset,
    }
    if (
      !isTextPoint(start, lengths) ||
      !Number.isSafeInteger(end.segmentIndex) ||
      !Number.isSafeInteger(end.offset) ||
      end.offset <= 0 ||
      end.offset > (lengths[end.segmentIndex] ?? -1) ||
      comparePoints(start, end) >= 0
    ) {
      continue
    }

    const previous = entries.at(-1)
    if (previous && comparePoints(previous.start, start) > 0) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      }
    }
    if (previous && comparePoints(previous.end, start) > 0) {
      return {
        status: 'unavailable',
        reason: 'ambiguous',
      }
    }
    ids.add(chunk.id)
    entries.push({
      start,
      end,
      chunkIndex: entries.length,
      chunkId: chunk.id,
    })
  }

  if (entries.length === 0) {
    return {
      status: 'unavailable',
      reason: 'not-ready',
    }
  }

  return {
    status: 'ready',
    spanAt: (chunkIndex) => {
      const entry = entries[chunkIndex]
      if (!entry) {
        return null
      }
      return {
        start: entry.start,
        end: entry.end,
      }
    },
    resolve(point) {
      if (!isTextPoint(point, lengths)) {
        return {
          status: 'unavailable',
          reason: 'invalid-point',
        }
      }

      let low = 0
      let high = entries.length
      while (low < high) {
        const middle = Math.floor((low + high) / 2)
        if (comparePoints(entries[middle].start, point) <= 0) {
          low = middle + 1
        } else {
          high = middle
        }
      }
      const entry = entries[low - 1]
      if (!entry || comparePoints(point, entry.end) >= 0) {
        return {
          status: 'unavailable',
          reason: 'unmapped',
        }
      }
      return {
        status: 'resolved',
        chunkIndex: entry.chunkIndex,
        chunkId: entry.chunkId,
      }
    },
  }
}

function isTextPoint(point: SourceTextPoint, lengths: readonly number[]): boolean {
  return Number.isSafeInteger(point.segmentIndex) && point.segmentIndex >= 0 &&
    Number.isSafeInteger(point.offset) && point.offset >= 0 &&
    point.offset < (lengths[point.segmentIndex] ?? 0)
}

function comparePoints(left: SourceTextPoint, right: SourceTextPoint): number {
  return left.segmentIndex - right.segmentIndex || left.offset - right.offset
}
