import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { FALLBACK_TTS_MODELS } from '../models'
import { DEFAULT_SILMA_NFE_STEP } from '../types'
import { AudioSetupPanel, type AudioSetupPanelProps } from './AudioSetupPanel'

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../../i18n', () => ({
  default: { t: (key: string) => key },
}))

const model = FALLBACK_TTS_MODELS[0]

function renderPanel(overrides: Partial<AudioSetupPanelProps> = {}): string {
  return renderToStaticMarkup(
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
    />,
  )
}

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
    const html = renderPanel({ debugEnabled: true, threadCount: 8 })

    expect(html).toContain('audio-setup-advanced-state')
    expect(html).toContain('⚠ tts.setup.threadCount')
    expect(html).toContain('tts.setup.diagnostics: tts.setup.on')
    expect(html).not.toContain('<details class="audio-setup-group audio-setup-advanced" open="">')
  })
})
