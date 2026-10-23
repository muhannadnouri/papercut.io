// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { setDebugEnabled } from '../../utils/debugFlags'
import { clearTtsDiagnostics, getTtsDiagnostics, logTtsTiming } from './TtsDiagnostics'

afterEach(() => {
  setDebugEnabled(false)
  clearTtsDiagnostics()
  vi.restoreAllMocks()
})

it('records detailed timings only when existing diagnostics are enabled', () => {
  clearTtsDiagnostics()
  setDebugEnabled(false)
  const clock = vi.spyOn(performance, 'now').mockReturnValue(107.3)
  logTtsTiming('[tts-playback] phase', 100, { bytes: 64 })
  expect(getTtsDiagnostics()).toHaveLength(0)
  setDebugEnabled(true)
  logTtsTiming('[tts-playback] phase', 100, { bytes: 64 })
  expect(getTtsDiagnostics()).toMatchObject([{ data: { elapsedMs: 7.3, bytes: 64 } }])
  expect(clock).toHaveBeenCalledOnce()
})
