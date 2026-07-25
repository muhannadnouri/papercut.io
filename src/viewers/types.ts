import type React from 'react'
import type { PdfTtsSourceSpan } from '../tts/types'
import type { SearchOpenTarget } from '../types/search'

export interface ViewerFindResult {
  currentIndex: number
  matchCount: number
}

export interface ViewerFindApi {
  search: (query: string) => void
  next: () => void
  previous: () => void
  clear: () => void
}

export interface ViewerProps {
  url: string
  format?: string
  content?: string
  contentRef?: React.RefObject<HTMLElement | null>
  toolbarTarget?: HTMLElement | null
  searchTarget?: SearchOpenTarget | null
  pdfTtsHighlightSpans?: PdfTtsSourceSpan[]
  onFindApiChange?: (api: ViewerFindApi | null) => void
  onFindResult?: (result: ViewerFindResult) => void
}

export interface ViewerPlugin {
  id: string
  canHandle: (url: string) => boolean
  Component: React.FC<ViewerProps>
}
