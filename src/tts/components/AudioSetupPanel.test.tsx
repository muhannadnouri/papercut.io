// @vitest-environment jsdom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import { FALLBACK_TTS_MODELS } from '../models'
import {
  DEFAULT_SILMA_NFE_STEP,
  DEFAULT_TTS_MODEL_ID,
  KOKORO_7M_MODEL_ID,
  NABRA_7M_MODEL_ID,
  PIPER_KAREEM_MODEL_ID,
  SILMA_MODEL_ID,
  SUPERTONIC_AR_MODEL_ID,
  SUPERTONIC_EN_MODEL_ID,
} from '../types'
import {
  AudioSetupPanel,
  type AudioSetupPanelProps,
} from './AudioSetupPanel'

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../../i18n', () => ({
  default: { t: (key: string) => key },
}))

const model = FALLBACK_TTS_MODELS[0]

function panel(overrides: Partial<AudioSetupPanelProps> = {}) {
  return (
    <AudioSetupPanel
      appliedThreadCount={null}
      defaultThreadCount={4}
      maxThreadCount={8}
      modelId={model.id}
      models={[model]}
      modelInstallProgress={null}
      modelStatus={null}
      onInstallModel={() => undefined}
      onModelChange={() => undefined}
      onPreviewStart={() => undefined}
      onSilmaNfeStepChange={() => undefined}
      onTextPreprocessorChange={() => undefined}
      onThreadCountChange={() => undefined}
      onVoiceChange={() => undefined}
      silmaNfeStep={DEFAULT_SILMA_NFE_STEP}
      textPreprocessor={model.defaultTextPreprocessor}
      textPreprocessors={model.textPreprocessors}
      threadCount={4}
      voice={model.defaultVoice}
      voices={model.voices}
      {...overrides}
    />
  )
}

function renderPanel(overrides: Partial<AudioSetupPanelProps> = {}): string {
  return renderToStaticMarkup(panel(overrides))
}

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

describe('AudioSetupPanel model recommendations', () => {
  it.each([
    [SUPERTONIC_EN_MODEL_ID, 'en', DEFAULT_TTS_MODEL_ID],
    [SUPERTONIC_AR_MODEL_ID, 'ar', PIPER_KAREEM_MODEL_ID],
    [KOKORO_7M_MODEL_ID, 'en', DEFAULT_TTS_MODEL_ID],
    [NABRA_7M_MODEL_ID, 'ar', PIPER_KAREEM_MODEL_ID],
  ])('labels the recommended model without replacing selected %s', async (modelId, language, recommendedId) => {
    const onModelChange = vi.fn()
    const element = document.createElement('div')
    const root = createRoot(element)
    const selected = FALLBACK_TTS_MODELS.find((item) => item.id === modelId)!
    const props = {
      modelId,
      voice: selected.defaultVoice,
      voices: selected.voices,
      onModelChange,
    }
    try {
      for (const models of [FALLBACK_TTS_MODELS, [...FALLBACK_TTS_MODELS].reverse()]) {
        await act(async () => root.render(panel({
          ...props,
          models,
        })))
        const select = element.querySelector<HTMLSelectElement>('.audio-field-model select')!
        expect(select.value).toBe(modelId)
        const recommended = Array.from(select.options).filter((option) => option.textContent?.includes('tts.setup.recommended'))
        expect(recommended.map((option) => option.value)).toEqual([recommendedId])
        expect(element.querySelector('.audio-field-voice option:checked')?.textContent)
          .toContain('tts.setup.recommended')
        expect(element.querySelector<HTMLSelectElement>('.audio-field-language select')?.value).toBe(language)
        expect(onModelChange).not.toHaveBeenCalled()
      }
    } finally {
      await act(async () => root.unmount())
    }
  })

  it.each([
    ['ar', DEFAULT_TTS_MODEL_ID, PIPER_KAREEM_MODEL_ID, ''],
    ['en', PIPER_KAREEM_MODEL_ID, DEFAULT_TTS_MODEL_ID, ''],
    ['ar', DEFAULT_TTS_MODEL_ID, SILMA_MODEL_ID, PIPER_KAREEM_MODEL_ID],
  ])('selects %s default from %s to %s (unavailable: %s)', async (language, modelId, expectedId, excludedId) => {
    const onModelChange = vi.fn()
    const element = document.createElement('div')
    const root = createRoot(element)
    try {
      await act(async () => root.render(panel({
        modelId,
        models: [...FALLBACK_TTS_MODELS].reverse().filter((item) => item.id !== excludedId),
        onModelChange,
      })))
      const select = element.querySelector<HTMLSelectElement>('.audio-field-language select')!
      await act(async () => {
        select.value = language
        select.dispatchEvent(new Event('change', { bubbles: true }))
      })
      expect(onModelChange).toHaveBeenCalledExactlyOnceWith(expectedId)
    } finally {
      await act(async () => root.unmount())
    }
  })

  it('does not label a fallback model as recommended when Kareem is unavailable', () => {
    const element = document.createElement('div')
    element.innerHTML = renderPanel({
      modelId: SUPERTONIC_AR_MODEL_ID,
      models: FALLBACK_TTS_MODELS.filter((item) => item.id !== PIPER_KAREEM_MODEL_ID),
    })
    expect(element.querySelector('.audio-field-model')?.textContent).not.toContain('tts.setup.recommended')
  })
})

describe('AudioSetupPanel Advanced disclosure', () => {
  it('starts collapsed while keeping the advanced controls in the document', () => {
    const html = renderPanel()

    expect(html).toContain('<details class="audio-setup-group audio-setup-advanced">')
    expect(html).toContain('<summary class="audio-setup-advanced-summary">')
    expect(html).toContain('<div class="audio-setup-advanced-grid">')
    expect(html).toContain('tts.setup.threads')
    expect(html).not.toContain('audio-setup-advanced-state')
  })

  it('summarizes active non-default settings without opening the disclosure', () => {
    const html = renderPanel({
      debugEnabled: true,
      threadCount: 8,
    })

    expect(html).toContain('audio-setup-advanced-state')
    expect(html).toContain('⚠ tts.setup.threadCount')
    expect(html).toContain('tts.setup.diagnostics: tts.setup.on')
    expect(html).not.toContain('<details class="audio-setup-group audio-setup-advanced" open="">')
  })
})

it.each([KOKORO_7M_MODEL_ID, NABRA_7M_MODEL_ID])('shows %s as a compact experiment with one valid voice', (id) => {
  const selected = FALLBACK_TTS_MODELS.find((item) => item.id === id)!
  const markup = renderPanel({
    models: FALLBACK_TTS_MODELS,
    modelId: id,
    voice: selected.defaultVoice,
    voices: selected.voices,
    textPreprocessor: selected.defaultTextPreprocessor,
    textPreprocessors: selected.textPreprocessors,
  })
  expect(markup).toContain(selected.name + ' · tts.setup.smallModel · settings.experimental')
  expect(markup).toContain('tts.setup.singleVoice')
  expect(markup).toContain(selected.voices[0].name + ' · tts.setup.recommended')
  expect(selected.voices).toHaveLength(1)
})

// A single named voice is a capability, independent of experimental model badges.
it('preserves Kareem’s voice name and explains its single-voice capability', () => {
  const selected = FALLBACK_TTS_MODELS.find((item) => item.id === PIPER_KAREEM_MODEL_ID)!
  const markup = renderPanel({
    modelId: selected.id,
    models: [selected],
    voice: selected.defaultVoice,
    voices: selected.voices,
  })
  expect(markup).toContain(selected.voices[0].name + ' · tts.setup.recommended')
  expect(markup).toContain('tts.setup.singleVoice')
  expect(markup).not.toContain('tts.setup.smallModel')
  expect(markup).not.toContain('settings.experimental')
})

it('does not show a single-voice hint for the full Kokoro roster', () => {
  expect(renderPanel()).not.toContain('tts.setup.singleVoice')
})

// Keep the diagnostic available for retry/reporting, without presenting a failed download as progress.
it('retains download errors without a percentage or progress meter', () => {
  const message = 'Failed to download: https://example.com/model.tar.bz2 (404 Not Found)'
  const markup = renderPanel({
    modelInstallProgress: {
      modelId: model.id,
      status: 'error',
      message,
      downloadedBytes: 0,
      totalBytes: 100,
      percent: 0,
    },
  })
  expect(markup).toContain(message)
  expect(markup).not.toContain('0%')
  expect(markup).not.toContain('role="progressbar"')
})
