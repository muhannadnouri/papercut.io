import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import type { TtsChunk } from '../types'

interface ImportedGraftDiagnosticContext {
  documentUrl?: string
  modelId?: string
  textPreprocessor?: string
}

export function chunksHaveDurableSourceSpans(chunks: TtsChunk[]): boolean {
  const speakableChunks = chunks.filter((chunk) => chunk.text.trim())
  return Boolean(speakableChunks.length && speakableChunks.every((chunk) => Boolean(chunk.sourceSpan)))
}

export function countChunkSourceSpans(chunks: TtsChunk[]): number {
  return chunks.filter((chunk) => Boolean(chunk.sourceSpan)).length
}

// Attach freshly rebuilt DOM spans only when restored HTML still chunks exactly
// like the imported bundle. Playback keeps using bundle identity either way.
export function graftImportedSourceSpans(
  importedChunks: TtsChunk[],
  rebuiltChunks: TtsChunk[],
  context: ImportedGraftDiagnosticContext = {},
): TtsChunk[] | null {
  if (importedChunks.length !== rebuiltChunks.length) {
    logImportedGraftFailure('chunk-count-mismatch', importedChunks, rebuiltChunks, -1, context)
    return null
  }

  const grafted: TtsChunk[] = []
  for (let index = 0; index < importedChunks.length; index++) {
    const imported = importedChunks[index]
    const rebuilt = rebuiltChunks[index]
    if (imported.id !== rebuilt.id) {
      logImportedGraftFailure('chunk-id-mismatch', importedChunks, rebuiltChunks, index, context)
      return null
    }
    if (imported.text !== rebuilt.text) {
      logImportedGraftFailure('chunk-text-mismatch', importedChunks, rebuiltChunks, index, context)
      return null
    }
    grafted.push({ ...imported, sourceSpan: rebuilt.sourceSpan })
  }

  logTtsDiagnostic('[tts-highlight] imported source-span graft ready', {
    chunks: importedChunks.length,
    rebuiltSourceSpans: rebuiltChunks.filter((chunk) => Boolean(chunk.sourceSpan)).length,
    modelId: context.modelId ?? '',
    textPreprocessor: context.textPreprocessor ?? '',
    documentUrl: context.documentUrl ?? '',
  })
  return grafted
}

// Keep import-graft diagnostics compact. Arabic failures often hide in Unicode
// details, so we include code point samples without storing large document text.
function logImportedGraftFailure(
  reason: string,
  importedChunks: TtsChunk[],
  rebuiltChunks: TtsChunk[],
  mismatchIndex: number,
  context: ImportedGraftDiagnosticContext,
): void {
  const imported = mismatchIndex >= 0 ? importedChunks[mismatchIndex] : undefined
  const rebuilt = mismatchIndex >= 0 ? rebuiltChunks[mismatchIndex] : undefined
  const importedText = imported?.text ?? ''
  const rebuiltText = rebuilt?.text ?? ''

  logTtsDiagnostic('[tts-highlight] imported source-span graft failed', {
    reason,
    mismatchIndex,
    importedChunks: importedChunks.length,
    rebuiltChunks: rebuiltChunks.length,
    importedId: imported?.id ?? '',
    rebuiltId: rebuilt?.id ?? '',
    importedLength: importedText.length,
    rebuiltLength: rebuiltText.length,
    sameAfterWhitespace: normalizeImportedGraftDiagnosticText(importedText) === normalizeImportedGraftDiagnosticText(rebuiltText),
    sameAfterNfc: importedText.normalize('NFC') === rebuiltText.normalize('NFC'),
    sameAfterNfkc: importedText.normalize('NFKC') === rebuiltText.normalize('NFKC'),
    importedPreview: previewImportedGraftDiagnosticText(importedText),
    rebuiltPreview: previewImportedGraftDiagnosticText(rebuiltText),
    importedCodePoints: previewImportedGraftCodePoints(importedText),
    rebuiltCodePoints: previewImportedGraftCodePoints(rebuiltText),
    modelId: context.modelId ?? '',
    textPreprocessor: context.textPreprocessor ?? '',
    documentUrl: context.documentUrl ?? '',
  }, 'warn')
}

function normalizeImportedGraftDiagnosticText(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}

function previewImportedGraftDiagnosticText(text: string): string {
  const normalized = normalizeImportedGraftDiagnosticText(text)
  return normalized.length <= 160 ? normalized : normalized.slice(0, 157).trimEnd() + '...'
}

function previewImportedGraftCodePoints(text: string): string {
  return Array.from(text)
    .slice(0, 32)
    .map((char) => 'U+' + (char.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0'))
    .join(' ')
}
