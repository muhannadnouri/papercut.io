// @vitest-environment jsdom
import { createHash } from 'node:crypto'
import { expect, it } from 'vitest'
import { buildNarrationMatchIndex } from './narrationText'
import { normalizeSegmentText } from './readableSegments'
import { chunkAudiobookSaveHtmlWithSpans, chunkSpeechText, AUDIOBOOK_SAVE_CHUNK_PROFILE, PLAYBACK_CHUNK_PROFILE, SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE } from '../utils/text'

const cases = [
  'A decimal 3.14 and U.S.A. initials. Another sentence!',
  '😀 A parenthetical (etc.). Next phrase.',
  ...Array.from('.,;:،؛，；：、!?؟。！？।॥', punctuation => 'Repeated words in a clause '.repeat(12) + punctuation + '“Another clause”' + punctuation + ' A longer ending '.repeat(20) + '.'),
  ...[')', ']', '"', "'", '”', '’', '»', '）', '】', '」', '』', '》'].flatMap(closer => [
    `😀 An abbreviation.${closer}. Next phrase.`, `An abbreviation.${closer}.`,
    `A.\u200f${closer}. Next.`, `A.${closer}\u064e. Next.`,
  ]),
  'بِدايَةٌ واضحةٌ. نِهايَةٌ واضحةٌ. '.repeat(15),
  'A very long uninterrupted sentence with ordinary word boundaries '.repeat(40),
  '. A leading terminator.  A spaced closer. ) . Another sentence.',
]

it('preserves pre-refactor narration text and spans across all shipped chunk profiles', () => {
  const result = [AUDIOBOOK_SAVE_CHUNK_PROFILE, PLAYBACK_CHUNK_PROFILE, SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE].map(profile =>
    cases.map(text => ({ speech: chunkSpeechText(text, profile), html: chunkAudiobookSaveHtmlWithSpans(`<h2>Heading</h2><p>${text}</p><p>Final paragraph.</p>`, profile) })))
  expect(createHash('sha256').update(JSON.stringify(result)).digest('hex')).toBe('479519f91412708ae305baea2a2393a26f36adf245fdb0823e2ebda1acd7f38a')
})

it('uses the generation grammar for compatibility without changing words or decimal values', () => {
  for (const profile of [AUDIOBOOK_SAVE_CHUNK_PROFILE, PLAYBACK_CHUNK_PROFILE, SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE]) {
    for (const text of cases) {
      expect(buildNarrationMatchIndex(normalizeSegmentText(text)).matchText).toBe(buildNarrationMatchIndex(chunkSpeechText(text, profile).join(' ')).matchText)
    }
  }
  expect(buildNarrationMatchIndex('another').matchText).not.toBe(buildNarrationMatchIndex('an other').matchText)
  expect(buildNarrationMatchIndex('1.2').matchText).not.toBe(buildNarrationMatchIndex('12').matchText)
})
