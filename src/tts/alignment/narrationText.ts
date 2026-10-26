/** Shared narration grammar. No reader, playback, scheduler or feature dependencies. */
const sentenceEndPattern = /[.!?؟。！？।॥]/u
const sentenceCloserPattern = /["'”’»)\]）】」』》]/u
const clauseSeparatorPattern = /[,;:،؛，；：、]/u
const sentenceBody = '[^' + sentenceEndPattern.source.slice(1, -1) + ']+'
const sentencePattern = new RegExp(sentenceBody + sentenceEndPattern.source + '+' + sentenceCloserPattern.source + '*|' + sentenceBody + '$', 'g')
const clausePattern = new RegExp('(' + clauseSeparatorPattern.source + '\\s*)')

export function splitNarrationSentences(text: string): string[] {
  return text.match(sentencePattern)?.map(sentence => sentence.trim()).filter(Boolean) ?? [text]
}

export function splitNarrationClauses(text: string): string[] {
  return text.split(clausePattern).reduce<string[]>((parts, part, index, source) => {
    if (index % 2 === 0) {
      parts.push(part + (source[index + 1] ?? ''))
    }
    return parts
  }, []).map(part => part.trim()).filter(Boolean)
}

export function isSentencePunctuationOrSpace(text: string): boolean {
  return Array.from(text).every(char => /\s/u.test(char) || sentenceEndPattern.test(char))
}

/** Baseline callers finish synchronously; only the feature schedules these pure steps. */
export function finishNarrationWork<T>(steps: Generator<void, T>): T {
  let step = steps.next()
  while (!step.done) {
    step = steps.next()
  }
  return step.value
}

export function buildNarrationMatchIndex(text: string) {
  return finishNarrationWork(narrationMatchIndexSteps(text))
}

// Imported audiobook bundles may only have chunk text, not durable DOM locators.
// The tolerant index keeps exact matching first, then removes Arabic-only visual
// marks and folds common Arabic/Persian codepoint variants for compatibility.
export function* narrationMatchIndexSteps(text: string): Generator<void, { matchText: string;
  matchTextOffsets: number[] }> {
  let matchText = ''
  let afterChunkSeparator = false
  const matchTextOffsets: number[] = []
  let batchUnits = 0

  // Walk the same raw sentence matches as generation; unmatched punctuation was not narrated.
  let found = false
  function* ranges() {
    for (const match of text.matchAll(sentencePattern)) {
      found = true
      const raw = match[0]
      yield {
        start: match.index + raw.length - raw.trimStart().length,
        end: match.index + raw.trimEnd().length,
      }
    }
    if (!found) {
      yield {
        start: 0,
        end: text.length,
      }
    }
  }
  for (const range of ranges()) {
    for (let offset = range.start; offset < range.end;) {
      const codePoint = text.codePointAt(offset)!
      const char = String.fromCodePoint(codePoint)
      const normalized = normalizeLocatorMatchChar(char)
      for (const outputChar of normalized) {
        if (outputChar === ' ' && afterChunkSeparator) {
          continue
        }
        afterChunkSeparator = sentenceEndPattern.test(outputChar) || clauseSeparatorPattern.test(outputChar) ||
          (afterChunkSeparator && sentenceCloserPattern.test(outputChar))
        matchText += outputChar
        for (let unit = 0; unit < outputChar.length; unit++) {
          matchTextOffsets.push(offset)
        }
      }
      offset += char.length
      batchUnits += char.length
      if (batchUnits >= 2048) {
        batchUnits = 0; yield
      }
    }
  }

  return {
    matchText,
    matchTextOffsets,
  }
}

function normalizeLocatorMatchChar(char: string): string {
  const normalized = char.normalize('NFKC')
  let output = ''
  for (const normalizedChar of normalized) {
    if (shouldDropArabicMatchChar(normalizedChar)) {
      continue
    }
    output += foldArabicMatchChar(normalizedChar)
  }
  return output
}

function shouldDropArabicMatchChar(char: string): boolean {
  const cp = char.codePointAt(0)
  if (cp === undefined) {
    return false
  }

  return (
    cp === 0x0640 || // tatweel
    cp === 0x0670 || // superscript alef
    cp === 0x200c || // ZWNJ
    cp === 0x200d || // ZWJ
    cp === 0x200e || // LRM
    cp === 0x200f || // RLM
    (cp >= 0x0610 && cp <= 0x061a) ||
    (cp >= 0x064b && cp <= 0x065f) ||
    (cp >= 0x06d6 && cp <= 0x06ed) ||
    (cp >= 0x202a && cp <= 0x202e) ||
    (cp >= 0x2066 && cp <= 0x2069)
  )
}

function foldArabicMatchChar(char: string): string {
  switch (char) {
    case '\u0622':
    case '\u0623':
    case '\u0625':
    case '\u0671':
      return '\u0627'
    case '\u0649':
    case '\u06CC':
      return '\u064A'
    case '\u06A9':
      return '\u0643'
    case '\u06C0':
      return '\u0647'
    default:
      return char
  }
}
