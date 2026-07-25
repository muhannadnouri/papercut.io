export const DEFAULT_TTS_MODEL_ID = 'sherpa-onnx/kokoro-multi-lang-v1_0'
export const KOKORO_ZH_MODEL_ID = 'sherpa-onnx/kokoro-multi-lang-v1_0-zh'
export const PIPER_KAREEM_MODEL_ID = 'sherpa-onnx/vits-piper-ar_JO-kareem-medium'
export const SUPERTONIC_EN_MODEL_ID = 'sherpa-onnx/supertonic-3-en'
export const SUPERTONIC_AR_MODEL_ID = 'sherpa-onnx/supertonic-3-ar'
export const TTS_AUDIO_CACHE_VERSION = 'native-save-v4-segmented'
export const NATIVE_TTS_DTYPE = 'native'
export const DEFAULT_TTS_VOICE = 'af_heart'
export const DEFAULT_TTS_SPEED = 1
export const TEXT_PREPROCESSOR_NONE = 'none'
export const LIBTASHKEEL_TEXT_PREPROCESSOR = 'libtashkeel-1.5.0'
export const SILMA_MODEL_ID = 'silma-ai/silma-tts'
export const DEFAULT_SILMA_NFE_STEP = 32
export const SILMA_NFE_STEP_OPTIONS = [64, 32, 16, 12, 8, 4] as const

export type TtsModelId = string
export type TtsVoice = string
export type TtsDtype = 'native'
export type TextPreprocessorId = string
export type SilmaNfeStep = typeof SILMA_NFE_STEP_OPTIONS[number]

export interface TtsVoiceInfo {
  id: TtsVoice
  name: string
}

export interface TextPreprocessorInfo {
  id: TextPreprocessorId
  name: string
  description: string
}

export interface TtsModelInfo {
  id: TtsModelId
  name: string
  family: string
  language: string
  languageLabel: string
  defaultVoice: TtsVoice
  voices: TtsVoiceInfo[]
  defaultTextPreprocessor: TextPreprocessorId
  textPreprocessors: TextPreprocessorInfo[]
}

export interface TtsOptions {
  modelId: TtsModelId
  voice: TtsVoice
  speed: number
  textPreprocessor?: TextPreprocessorId
  dtype?: TtsDtype
  threadCount?: number
  silmaNfeStep?: number
  documentUrl?: string
  title?: string
}

export function resolveTtsDtype(options: Pick<TtsOptions, 'dtype'>): TtsDtype {
  return options.dtype ?? NATIVE_TTS_DTYPE
}

export function resolveTextPreprocessor(
  options: Pick<TtsOptions, 'textPreprocessor'>,
): TextPreprocessorId {
  return options.textPreprocessor ?? TEXT_PREPROCESSOR_NONE
}

export function resolveSilmaNfeStep(
  options: Pick<TtsOptions, 'silmaNfeStep'>,
): SilmaNfeStep {
  return SILMA_NFE_STEP_OPTIONS.includes(options.silmaNfeStep as SilmaNfeStep)
    ? options.silmaNfeStep as SilmaNfeStep
    : DEFAULT_SILMA_NFE_STEP
}

export interface TtsChunkSourceSpan {
  startSegmentIndex: number
  startOffset: number
  endSegmentIndex: number
  endOffset: number
}

export interface PdfTtsSourceSpan {
  pageIndex: number
  blockOrder: number
  startOffset: number
  endOffset: number
}

export interface TtsChunk {
  id: string
  text: string
  textHash?: string | null
  sourceSpan?: TtsChunkSourceSpan
  // Runtime-only PDF coordinates. Native manifests keep sourceSpan as the
  // durable identity and rebuild these bounded active-view ranges on reopen.
  pdfSourceSpans?: PdfTtsSourceSpan[]
}
