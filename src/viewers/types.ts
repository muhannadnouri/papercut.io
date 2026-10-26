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

export interface ViewerViewportBounds {
  top: number
  bottom: number
  left: number
  right: number
}

export type ViewerTextTargetResult = {
  status: 'captured'
  point: { format: 'html'; segmentIndex: number; offset: number }
  segmentTexts: readonly string[]
  segmentLengths: readonly number[]
  isCurrent: () => boolean
} | {
  status: 'captured'
  point: { format: 'pdf'; pageIndex: number; blockOrder: number; offset: number }
  sourceText: string
  isCurrent: () => boolean
} | {
  status: 'unavailable'
  reason: 'not-ready' | 'no-visible-text' | 'no-selection' | 'unmapped' | 'stale'
}

export interface ViewerTextTargetApi {
  captureSelection?: () => ViewerTextTargetResult
  captureViewport: () => Promise<ViewerTextTargetResult>
}

export interface PdfBookmarkLocation {
  pageNumber: number
  left: number
  top: number
}

export interface HtmlBookmarkLocation {
  textOffset: number
}

export type ViewerBookmarkLocation = PdfBookmarkLocation | HtmlBookmarkLocation

export interface ViewerBookmarkApi {
  capture: () => ViewerBookmarkLocation | null
  isCurrent: (location: ViewerBookmarkLocation) => boolean
  isPastStart: () => boolean
  restore: (location: ViewerBookmarkLocation) => void
  scrollToTop: () => void
  subscribe: (listener: () => void) => () => void
}

export interface AudioFollowPolicy {
  active?: boolean
  allowsReveal: () => boolean
  revision: number
}

export interface ViewerProps {
  url: string
  format?: string
  content?: string
  contentRef?: React.RefObject<HTMLElement | null>
  toolbarTarget?: HTMLElement | null
  searchTarget?: SearchOpenTarget | null
  audioFollowPolicy?: AudioFollowPolicy
  onReaderBrowse?: () => void
  pdfTtsHighlightSpans?: PdfTtsSourceSpan[]
  fullscreen?: boolean
  onBookmarkApiChange?: (api: ViewerBookmarkApi | null) => void
  onFindApiChange?: (api: ViewerFindApi | null) => void
  onFindResult?: (result: ViewerFindResult) => void
  onTextTargetApiChange?: (api: ViewerTextTargetApi | null) => void
  getTextTargetViewport?: () => ViewerViewportBounds | null
  onFullscreenChange?: () => void
}

export interface ViewerPlugin {
  id: string
  canHandle: (url: string) => boolean
  Component: React.FC<ViewerProps>
}
