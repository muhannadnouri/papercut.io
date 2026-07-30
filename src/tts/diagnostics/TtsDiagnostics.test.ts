import { describe, expect, it } from 'vitest'
import { summarizeTtsCapabilities } from './TtsDiagnostics'

describe('summarizeTtsCapabilities', () => {
  it('keeps execution-provider probe results visible in diagnostics', () => {
    expect(summarizeTtsCapabilities({
      available: true,
      backend: 'sherpa-onnx',
      compiledExecutionProviders: ['cpu', 'coreml'],
      defaultExecutionProvider: 'coreml',
      defaultThreadCount: 1,
      executionProviderProbeError: null,
      maxThreadCount: 8,
      models: [{ id: 'test-model' }],
      platform: 'ios',
      reason: 'ready',
    })).toMatchObject({
      compiledExecutionProviders: 'cpu, coreml',
      defaultExecutionProvider: 'coreml',
      executionProviderProbeError: '',
      modelCount: 1,
    })
  })
})
