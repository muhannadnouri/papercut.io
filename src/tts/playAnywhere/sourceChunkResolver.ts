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
    resolve: (point: SourceTextPoint) => ChunkTargetResult }
  | UnavailableTarget

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
    const span = chunk.sourceSpan
    // shortcut: require complete spans; allow partial coverage only with explicit gap metadata.
    if (!span) {
      return {
        status: 'unavailable',
        reason: 'unmapped',
      }
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
      comparePoints(start, end) >= 0 ||
      !chunk.id.trim() || ids.has(chunk.id)
    ) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      }
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
