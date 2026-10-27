import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { runPreparationWork } from './preparationWork'

vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsDiagnostic: vi.fn() }))
afterEach(() => vi.useRealTimers())

it('defers cold work and lets another event cancel between bounded batches', async () => {
  vi.useFakeTimers()
  const abort = new AbortController()
  let visited = 0
  let closed = false
  function* work() {
    try {
      while (visited < 10000) {
        visited++
        yield
      }
      return 42
    } finally {
      closed = true
    }
  }
  const result = runPreparationWork(work(), () => true, [abort.signal])
  const rejected = expect(result).rejects.toMatchObject({ name: 'AbortError' })
  expect(visited).toBe(0)
  setTimeout(() => abort.abort(), 0)
  await vi.runAllTimersAsync()
  await rejected
  expect(visited).toBeGreaterThan(0)
  expect(visited).toBeLessThanOrEqual(128)
  expect(closed).toBe(true)
  expect(vi.getTimerCount()).toBe(0)
})

it('completes all steps and removes abort listeners and timers', async () => {
  vi.useFakeTimers()
  const signal = new AbortController().signal
  const remove = vi.spyOn(signal, 'removeEventListener')
  function* work() {
    for (let i = 0; i < 500; i++) {
      yield
    }
    return 42
  }
  const result = runPreparationWork(work(), () => true, [signal])
  await vi.runAllTimersAsync()
  expect(await result).toBe(42)
  expect(remove).toHaveBeenCalledWith('abort', expect.any(Function))
  expect(vi.getTimerCount()).toBe(0)
})

it.each(['iterator', 'guard', 'close'])('contains %s exceptions and cleans up', async phase => {
  vi.useFakeTimers()
  const controller = new AbortController()
  function* work() {
    if (phase === 'iterator') {
      throw new Error('iterator')
    }
    yield
    return 42
  }
  const steps = work()
  if (phase === 'close') {
    vi.spyOn(steps, 'return').mockImplementation(() => {
      throw new Error('close')
    })
  }
  let guards = 0
  const result = runPreparationWork(steps, () => {
    if (phase === 'guard') {
      throw new Error('guard')
    }
    return phase !== 'close' || ++guards < 3
  }, [controller.signal])
  const rejected = expect(result).rejects.toThrow(phase)
  await vi.runAllTimersAsync()
  await rejected
  expect(vi.getTimerCount()).toBe(0)
})

it('does not run an already cancelled job or complete after an abort inside a step', async () => {
  vi.useFakeTimers()
  const controller = new AbortController()
  const next = vi.fn()
  function* work() {
    next()
    yield
    return 42
  }
  controller.abort()
  await expect(runPreparationWork(work(), () => true, [controller.signal])).rejects.toMatchObject({ name: 'AbortError' })
  expect(next).not.toHaveBeenCalled()
  const other = new AbortController()
  function* reentrant() {
    other.abort()
    yield
    next()
    return 42
  }
  const result = runPreparationWork(reentrant(), () => true, [other.signal])
  const rejected = expect(result).rejects.toMatchObject({ name: 'AbortError' })
  await vi.runAllTimersAsync()
  await rejected
  expect(next).not.toHaveBeenCalled()
  expect(vi.getTimerCount()).toBe(0)
})
