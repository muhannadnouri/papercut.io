// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { getImportedAudiobookMetadata, getNativeSavedAudiobookChunk } from './nativeTts'
import { logTtsTiming } from '../diagnostics/TtsDiagnostics'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsTiming: vi.fn() }))
beforeEach(() => {
  vi.clearAllMocks()
  Object.defineProperty(window, '__TAURI_INTERNALS__', { configurable: true, value: {} })
})
afterEach(() => { Reflect.deleteProperty(window, '__TAURI_INTERNALS__') })
const chunk = { id: 'private-id', text: 'Private book prose.' }
const options = { modelId: 'kokoro', voice: 'af_heart', speed: 1 }

it('separates read round trip and byte conversion without logging request data', async () => {
  vi.mocked(invoke).mockResolvedValue({ wavBase64: 'AQID', wavBytes: 3 })
  const result = await getNativeSavedAudiobookChunk('/private-book.html', chunk, 4, options)
  expect([...new Uint8Array(result!.wav)]).toEqual([1, 2, 3])
  expect(logTtsTiming).toHaveBeenCalledWith('[tts-native] read round trip', expect.any(Number), {
    command: 'tts_get_native_audiobook_chunk', ok: true,
  })
  expect(logTtsTiming).toHaveBeenCalledWith('[tts-playback] WAV base64 conversion', expect.any(Number), { bytes: 3 })
  const events = JSON.stringify(vi.mocked(logTtsTiming).mock.calls)
  expect(events).not.toContain('Private')
  expect(events).not.toContain('private-id')
  expect(events).not.toContain('private-book')
  expect(events).not.toContain('AQID')
})

it('retains structured command errors while timing failed native reads', async () => {
  vi.mocked(invoke).mockRejectedValue({ code: 'audiobook-cache-mismatch', message: 'private detail' })
  await expect(getImportedAudiobookMetadata('/private-book.html')).rejects.toMatchObject({
    code: 'audiobook-cache-mismatch', message: 'private detail',
  })
  expect(logTtsTiming).toHaveBeenCalledWith('[tts-native] read round trip', expect.any(Number), {
    command: 'tts_get_imported_audiobook_metadata', ok: false,
  })
  expect(JSON.stringify(vi.mocked(logTtsTiming).mock.calls)).not.toContain('private')
})

it('still reports an unavailable chunk when the native payload cannot be converted', async () => {
  vi.mocked(invoke).mockResolvedValue({ wavBase64: '%' })
  expect(await getNativeSavedAudiobookChunk('/private-book.html', chunk, 4, options)).toBeNull()
  expect(vi.mocked(logTtsTiming).mock.calls.map(call => call[0])).toEqual(['[tts-native] read round trip'])
})
