import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'

/** Feature-owned timers; shared narration stays synchronous and scheduler-free. */
export function runPreparationWork<T>(steps: Generator<void, T>, isCurrent: () => boolean, signals: readonly (AbortSignal | undefined)[]): Promise<T> {
  return new Promise((resolve, reject) => {
    let timer: ReturnType<typeof setTimeout> | undefined
    let settled = false; let batches = 0; let maxBatchMs = 0
    let advancing = false
    const started = performance.now()
    const cleanup = () => {
      clearTimeout(timer)
      for (const signal of signals) {
        signal?.removeEventListener('abort', cancel)
      }
    }
    const cancel = () => {
      if (settled) {
        return
      }
      settled = true; cleanup()
      try {
        if (!advancing) {
          steps.return(undefined as never)
        }
      } catch (error) {
        reject(error); return
      }
      reject(new DOMException('Preparation cancelled', 'AbortError'))
    }
    const pump = () => {
      timer = undefined
      const batchStart = performance.now()
      try {
        if (!isCurrent() || signals.some(signal => signal?.aborted)) {
          cancel(); return
        }
        if (settled) {
          return
        }
        let step: IteratorResult<void, T> | undefined
        // A count cap also guarantees yielding when timer clocks are coarse or mocked.
        for (let count = 0; count < 128; count++) {
          if (!isCurrent()) {
            cancel(); return
          }
          if (settled) {
            return
          }
          advancing = true
          try {
            step = steps.next()
          } finally {
            advancing = false
          }
          if (settled) {
            steps.return(undefined as never); return
          }
          if (step.done || performance.now() - batchStart >= 8) {
            break
          }
        }
        batches++; maxBatchMs = Math.max(maxBatchMs, performance.now() - batchStart)
        if (step?.done) {
          settled = true; cleanup()
          logTtsDiagnostic('[play-anywhere] preparation completed', {
            batches,
            maxBatchMs,
            elapsedMs: performance.now() - started,
          })
          resolve(step.value)
        } else {
          timer = setTimeout(pump, 0)
        }
      } catch (error) {
        settled = true; cleanup()
        try {
          steps.return(undefined as never)
        } catch (closeError) {
          reject(closeError); return
        }
        reject(error)
      }
    }
    for (const signal of signals) {
      signal?.addEventListener('abort', cancel, { once: true })
    }
    if (signals.some(signal => signal?.aborted)) {
      cancel()
    } else {
      timer = setTimeout(pump, 0)
    }
  })
}
