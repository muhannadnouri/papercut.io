import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import type { TtsChunkSummary, TtsPlayerState } from '../hooks/useTtsPlayer'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { formatSpeedLabel } from '../utils/format'
import { SavedAudiobooksMenu } from './SavedAudiobooksMenu'
import './AudioControls.css'

interface AudioControlsProps {
  audiobookState: AudiobookCacheState
  canPlayAudiobook: boolean
  canSaveAudiobook: boolean
  canSkipBackward: boolean
  canSkipForward: boolean
  isPdf: boolean
  saveInProgress: boolean
  onManageSave: () => void
  onPause: () => void
  onRead: () => void
  onResume: () => void
  onSelectSavedAudiobook: (record: SavedAudiobookRecord) => void
  onJumpToChunk: (index: number) => void
  onPlaybackRateChange: (rate: number) => void
  onSave: () => void
  onSkipBackward: () => void
  onSkipForward: () => void
  onStop: () => void
  onWordHighlightEnabledChange: (enabled: boolean) => void
  playbackDurationSec?: number
  playbackNotice?: string
  playbackRate: number
  savedAudiobooks: SavedAudiobookRecord[]
  selectedAudiobookId: string | null
  ttsState: TtsPlayerState
  wordHighlightEnabled: boolean
}

type AudioIconName = 'play' | 'pause' | 'resume' | 'stop' | 'back' | 'forward' | 'save' | 'menu'
const PLAYBACK_RATE_OPTIONS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3]

export function AudioControls({
  audiobookState,
  canPlayAudiobook,
  canSaveAudiobook,
  canSkipBackward,
  canSkipForward,
  isPdf,
  saveInProgress,
  onManageSave,
  onPause,
  onRead,
  onResume,
  onSelectSavedAudiobook,
  onJumpToChunk,
  onPlaybackRateChange,
  onSave,
  onSkipBackward,
  onSkipForward,
  onStop,
  onWordHighlightEnabledChange,
  playbackDurationSec,
  playbackNotice,
  playbackRate,
  savedAudiobooks,
  selectedAudiobookId,
  ttsState,
  wordHighlightEnabled,
}: AudioControlsProps) {
  const { t } = useTranslation()
  const controlsRef = useRef<HTMLElement | null>(null)
  const [chunkMenuOpen, setChunkMenuOpen] = useState(false)
  const isActive = ttsState.status === 'playing' ||
    ttsState.status === 'loading'
  const isPaused = ttsState.status === 'paused'
  const showFloatingPlayback = isActive || isPaused
  const isPreparingSave = saveInProgress && audiobookState.status === 'checking'
  const isSaving = saveInProgress && audiobookState.status === 'saving'
  const audiobookPercent = audiobookState.totalChunks > 0
    ? Math.round((audiobookState.cachedChunks / audiobookState.totalChunks) * 100)
    : 0
  const visibleChunkIndex = ttsState.pendingChunkIndex ?? ttsState.currentChunkIndex
  const currentChunkNumber = visibleChunkIndex === null
    ? Math.min(ttsState.chunksPlayed + 1, ttsState.chunksTotal)
    : visibleChunkIndex + 1
  const chunkTotal = ttsState.chunksTotal || Math.max(ttsState.chunksGenerated, ttsState.chunksPlayed)
  const chunkPercent = Math.round(ttsState.currentChunkProgress * 100)
  const showPlaybackMenuButton = showFloatingPlayback
  const showPlaybackStatus = ttsState.status !== 'idle'
  const playbackRateLabel = formatSpeedLabel(playbackRate)

  const handleChunkSelect = useCallback((index: number) => {
    setChunkMenuOpen(false)
    onJumpToChunk(index)
  }, [onJumpToChunk])

  const handlePlaybackRateChange = useCallback(() => {
    onPlaybackRateChange(nextPlaybackRate(playbackRate))
  }, [onPlaybackRateChange, playbackRate])

  useEffect(() => {
    if (!chunkMenuOpen) return

    function handlePointerDown(event: PointerEvent) {
      const root = controlsRef.current
      if (!root || root.contains(event.target as Node)) return
      setChunkMenuOpen(false)
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setChunkMenuOpen(false)
    }

    document.addEventListener('pointerdown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [chunkMenuOpen])

  return (
    <section ref={controlsRef} className="audio-controls" aria-label={t('tts.controls.ariaLabel')}>
      <div className="audio-compact-row">
        {savedAudiobooks.length > 0 && (
          <SavedAudiobooksMenu
            records={savedAudiobooks}
            selectedId={selectedAudiobookId}
            onSelect={onSelectSavedAudiobook}
          />
        )}
        {!showFloatingPlayback && canPlayAudiobook && (
          <button className="audio-icon-btn audio-primary-btn" onClick={onRead} aria-label={t('tts.controls.playSaved')} title={t('tts.controls.playSaved')}>
            <AudioIcon name="play" />
          </button>
        )}
        {!isPdf && renderSaveButton()}
      </div>

      {showPlaybackMenuButton && chunkMenuOpen && (
        <ChunkMenu
          chunks={ttsState.chunkSummaries}
          currentChunkIndex={ttsState.currentChunkIndex}
          chunksTotal={ttsState.chunksTotal}
          playbackDurationSec={playbackDurationSec}
          wordHighlightEnabled={wordHighlightEnabled}
          onSelect={handleChunkSelect}
          onWordHighlightEnabledChange={onWordHighlightEnabledChange}
        />
      )}

      {showFloatingPlayback && (
        <div className="audio-floating-playback" aria-label={t('tts.controls.playbackControls')}>
          <button className="audio-icon-btn" onClick={onSkipBackward} disabled={!canSkipBackward} aria-label={t('tts.controls.previousChunk')} title={t('tts.controls.previousChunkTitle')}>
            <AudioIcon name="back" />
          </button>
          {isPaused ? (
            <button className="audio-icon-btn audio-primary-btn" onClick={onResume} aria-label={t('tts.controls.resume')} title={t('tts.controls.resumeTitle')}>
              <AudioIcon name="resume" />
            </button>
          ) : (
            <button className="audio-icon-btn audio-primary-btn" onClick={onPause} disabled={ttsState.status === 'loading'} aria-label={t('tts.controls.pause')} title={t('tts.controls.pauseTitle')}>
              <AudioIcon name="pause" />
            </button>
          )}
          <button className="audio-icon-btn" onClick={onSkipForward} disabled={!canSkipForward} aria-label={t('tts.controls.nextChunk')} title={t('tts.controls.nextChunkTitle')}>
            <AudioIcon name="forward" />
          </button>
          {showPlaybackMenuButton && (
            <button
              className={'audio-icon-btn audio-menu-btn' + (chunkMenuOpen ? ' audio-menu-btn-open' : '')}
              onClick={() => setChunkMenuOpen((value) => !value)}
              aria-label={chunkMenuOpen ? t('tts.controls.hideMenu') : t('tts.controls.showMenu')}
              aria-expanded={chunkMenuOpen}
              title={t('tts.controls.menuTitle')}
            >
              <AudioIcon name="menu" />
            </button>
          )}
          <button
            type="button"
            className="audio-rate-control"
            onClick={handlePlaybackRateChange}
            aria-label={t('tts.controls.playbackSpeed', { speed: playbackRateLabel })}
            title={t('tts.controls.playbackSpeedTitle')}
          >
            {playbackRateLabel}
          </button>
          <button className="audio-icon-btn" onClick={onStop} aria-label={t('tts.controls.stop')} title={t('tts.controls.stopTitle')}>
            <AudioIcon name="stop" />
          </button>
          {showPlaybackStatus && (
            <div className={'audio-floating-status tts-status-' + ttsState.status} dir="auto">
              <span>{ttsState.status === 'error'
                ? ttsState.message
                : t('tts.controls.chunkProgress', { current: currentChunkNumber || 0, total: chunkTotal })}</span>
              {ttsState.status !== 'error' && playbackNotice && (
                <span>{playbackNotice}</span>
              )}
              {ttsState.status !== 'error' && ttsState.currentChunkDuration > 0 && (
                <span>{formatTtsTime(ttsState.currentChunkTime)} / {formatTtsTime(ttsState.currentChunkDuration)}</span>
              )}
              {ttsState.status !== 'error' && (
                <div className="tts-meter" aria-label={t('tts.controls.currentChunkPercent', { percent: chunkPercent })}>
                  <span style={{ width: chunkPercent + '%' }} />
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  )

  function renderSaveButton() {
    if (isPreparingSave) {
      return (
        <button className="audio-icon-btn" disabled aria-label={t('tts.controls.preparingSave')} title={t('tts.controls.preparingSave')}>
          <AudioIcon name="save" />
        </button>
      )
    }

    if (isSaving) {
      return (
        <button
          className="audio-icon-btn"
          onClick={onManageSave}
          aria-label={t('tts.controls.saveProgress', { percent: audiobookPercent })}
          title={t('tts.controls.saveProgressTitle', { percent: audiobookPercent })}
        >
          <span className="spinner audio-save-spinner" />
        </button>
      )
    }

    const saveLabel = savedAudiobooks.length > 0
      ? t('tts.controls.saveCurrentSetup')
      : t('tts.controls.save')
    const buttonLabel = audiobookState.complete ? t('tts.controls.savedForVoice') : saveLabel

    return (
      <button
        className={'audio-icon-btn' + (audiobookState.complete ? ' audio-save-complete' : '')}
        onClick={onSave}
        disabled={!canSaveAudiobook || audiobookState.complete}
        aria-label={buttonLabel}
        title={buttonLabel}
      >
        <AudioIcon name="save" />
      </button>
    )
  }

}

function nextPlaybackRate(currentRate: number): number {
  const currentIndex = PLAYBACK_RATE_OPTIONS.findIndex((rate) => Math.abs(rate - currentRate) < 0.001)
  if (currentIndex === -1 || currentIndex === PLAYBACK_RATE_OPTIONS.length - 1) return PLAYBACK_RATE_OPTIONS[0]
  return PLAYBACK_RATE_OPTIONS[currentIndex + 1]
}

interface ChunkMenuProps {
  chunks: TtsChunkSummary[]
  currentChunkIndex: number | null
  chunksTotal: number
  playbackDurationSec?: number
  wordHighlightEnabled: boolean
  onSelect: (index: number) => void
  onWordHighlightEnabledChange: (enabled: boolean) => void
}

const CHUNK_ROW_HEIGHT = 44
const CHUNK_MENU_VISIBLE_ROWS = 12
const CHUNK_MENU_OVERSCAN = 6

const ChunkMenu = memo(function ChunkMenu({
  chunks,
  currentChunkIndex,
  chunksTotal,
  playbackDurationSec,
  wordHighlightEnabled,
  onSelect,
  onWordHighlightEnabledChange,
}: ChunkMenuProps) {
  const { t } = useTranslation()
  const listRef = useRef<HTMLDivElement | null>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const firstVisibleIndex = Math.max(
    0,
    Math.floor(scrollTop / CHUNK_ROW_HEIGHT) - CHUNK_MENU_OVERSCAN,
  )
  const lastVisibleIndex = Math.min(
    chunks.length,
    firstVisibleIndex + CHUNK_MENU_VISIBLE_ROWS + CHUNK_MENU_OVERSCAN * 2,
  )
  const visibleChunks = chunks.slice(firstVisibleIndex, lastVisibleIndex)

  useEffect(() => {
    if (currentChunkIndex === null || !listRef.current) return
    const list = listRef.current
    const nextScrollTop = Math.max(
      0,
      currentChunkIndex * CHUNK_ROW_HEIGHT - (list.clientHeight - CHUNK_ROW_HEIGHT) / 2,
    )
    list.scrollTop = nextScrollTop
    setScrollTop(nextScrollTop)
  }, [currentChunkIndex])

  return (
    <div className="audio-chunk-menu" aria-label={t('tts.controls.menuTitle')}>
      <div className="audio-chunk-menu-header">
        <span>{t('tts.controls.playback')}</span>
        <button
          type="button"
          className="audio-word-highlight-toggle"
          onClick={() => onWordHighlightEnabledChange(!wordHighlightEnabled)}
          aria-pressed={wordHighlightEnabled}
          aria-label={t('tts.controls.wordHighlightState', {
            state: wordHighlightEnabled ? t('tts.setup.on') : t('tts.setup.off'),
          })}
        >
          {t('tts.controls.wordHighlight')} <span className="audio-beta-tag">{t('tts.controls.experimental')}</span>: {wordHighlightEnabled ? t('tts.setup.on') : t('tts.setup.off')}
        </button>
      </div>
      {chunks.length > 1 && (
        <>
          <div className="audio-chunk-menu-subheader">
            <span>{t('tts.controls.jumpTo')}</span>
            <span>{t('tts.controls.chunkCount', { count: chunks.length })}</span>
          </div>
          <div
            ref={listRef}
            className="audio-chunk-list"
            onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
          >
            <div className="audio-chunk-virtual-space" style={{ height: chunks.length * CHUNK_ROW_HEIGHT }}>
              <div
                className="audio-chunk-window"
                style={{ transform: `translateY(${firstVisibleIndex * CHUNK_ROW_HEIGHT}px)` }}
              >
                {visibleChunks.map((chunk) => {
                  const isCurrent = chunk.index === currentChunkIndex
                  const estimatedStart = estimateChunkStart(chunk.index, chunksTotal, playbackDurationSec)
                  return (
                    <button
                      key={chunk.chunkId}
                      className={'audio-chunk-item' + (isCurrent ? ' audio-chunk-item-current' : '')}
                      style={{ height: CHUNK_ROW_HEIGHT }}
                      onClick={() => onSelect(chunk.index)}
                      title={chunk.textPreview}
                    >
                      <span className="audio-chunk-time">{estimatedStart === null ? '--:--' : formatTtsTime(estimatedStart)}</span>
                      <span className="audio-chunk-text" dir="auto">{chunk.textPreview}</span>
                      <span className="audio-chunk-number">{chunk.index + 1}</span>
                    </button>
                  )
                })}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
})

function AudioIcon({ name }: { name: AudioIconName }) {
  return (
    <svg className="audio-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {renderIconPath(name)}
    </svg>
  )
}

function renderIconPath(name: AudioIconName) {
  switch (name) {
    case 'play':
    case 'resume':
      return <path d="M8 5v14l11-7z" />
    case 'pause':
      return <path d="M7 5h4v14H7zm6 0h4v14h-4z" />
    case 'stop':
      return <path d="M7 7h10v10H7z" />
    case 'back':
      return <path d="M11 6v12l-8.5-6zm10 0v12l-8.5-6z" />
    case 'forward':
      return <path d="M13 6v12l8.5-6zM3 6v12l8.5-6z" />
    case 'save':
      return <path d="M5 3h12l2 2v16H5zM8 3v6h8V3M8 18h8v-5H8z" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinejoin="round" />
    case 'menu':
      return <path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" />
  }
}

function formatTtsTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0:00'
  const rounded = Math.floor(seconds)
  const hours = Math.floor(rounded / 3600)
  const minutes = Math.floor((rounded % 3600) / 60)
  const remainingSeconds = rounded % 60
  if (hours > 0) return hours + ':' + String(minutes).padStart(2, '0') + ':' + String(remainingSeconds).padStart(2, '0')
  return minutes + ':' + String(remainingSeconds).padStart(2, '0')
}

function estimateChunkStart(index: number, totalChunks: number, totalDurationSec?: number): number | null {
  if (!totalDurationSec || totalDurationSec <= 0 || totalChunks <= 0) return null
  return Math.max(0, (totalDurationSec / totalChunks) * index)
}
