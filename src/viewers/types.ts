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

export interface ViewerBookmarkLocation {
  pageNumber: number
  left: number
  top: number
}

export interface ViewerBookmarkApi {
  capture: () => ViewerBookmarkLocation
  isCurrent: (location: ViewerBookmarkLocation) => boolean
  isPastStart: () => boolean
  restore: (location: ViewerBookmarkLocation) => void
  scrollToTop: () => void
  subscribe: (listener: () => void) => () => void
}

export interface ViewerProps {
  url: string
  format?: string
  content?: string
  contentRef?: React.RefObject<HTMLElement | null>
  toolbarTarget?: HTMLElement | null
  searchTarget?: SearchOpenTarget | null
  pdfTtsHighlightSpans?: PdfTtsSourceSpan[]
  onBookmarkApiChange?: (api: ViewerBookmarkApi | null) => void
  onFindApiChange?: (api: ViewerFindApi | null) => void
  onFindResult?: (result: ViewerFindResult) => void
}

export interface ViewerPlugin {
  id: string
  canHandle: (url: string) => boolean
  Component: React.FC<ViewerProps>
}
