import { memo, useCallback, useEffect, useRef, useState } from 'react'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import type { TtsChunkSummary, TtsPlayerState } from '../hooks/useTtsPlayer'
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
  onJumpToChunk: (index: number) => void
  onSave: () => void
  onSkipBackward: () => void
  onSkipForward: () => void
  onStop: () => void
  playbackDurationSec?: number
  playbackNotice?: string
  ttsState: TtsPlayerState
}

type AudioIconName = 'play' | 'pause' | 'resume' | 'stop' | 'back' | 'forward' | 'save' | 'menu'

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
  onJumpToChunk,
  onSave,
  onSkipBackward,
  onSkipForward,
  onStop,
  playbackDurationSec,
  playbackNotice,
  ttsState,
}: AudioControlsProps) {
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
  const showChunkMenuButton = showFloatingPlayback && ttsState.chunkSummaries.length > 1
  const showPlaybackStatus = ttsState.status !== 'idle'

  const handleChunkSelect = useCallback((index: number) => {
    setChunkMenuOpen(false)
    onJumpToChunk(index)
  }, [onJumpToChunk])

  return (
    <section className="audio-controls" aria-label="Audiobook controls">
      <div className="audio-compact-row">
        {!showFloatingPlayback && canPlayAudiobook && (
          <button className="audio-icon-btn audio-primary-btn" onClick={onRead} aria-label="Play saved audiobook" title="Play saved audiobook">
            <AudioIcon name="play" />
          </button>
        )}
        {!isPdf && renderSaveButton()}
      </div>

      {showChunkMenuButton && chunkMenuOpen && (
        <ChunkMenu
          chunks={ttsState.chunkSummaries}
          currentChunkIndex={ttsState.currentChunkIndex}
          chunksTotal={ttsState.chunksTotal}
          playbackDurationSec={playbackDurationSec}
          onSelect={handleChunkSelect}
        />
      )}

      {showFloatingPlayback && (
        <div className="audio-floating-playback" aria-label="Playback controls">
          <button className="audio-icon-btn" onClick={onSkipBackward} disabled={!canSkipBackward} aria-label="Previous audiobook chunk" title="Previous chunk">
            <AudioIcon name="back" />
          </button>
          {isPaused ? (
            <button className="audio-icon-btn audio-primary-btn" onClick={onResume} aria-label="Resume audiobook" title="Resume">
              <AudioIcon name="resume" />
            </button>
          ) : (
            <button className="audio-icon-btn audio-primary-btn" onClick={onPause} disabled={ttsState.status === 'loading'} aria-label="Pause audiobook" title="Pause">
              <AudioIcon name="pause" />
            </button>
          )}
          <button className="audio-icon-btn" onClick={onSkipForward} disabled={!canSkipForward} aria-label="Next audiobook chunk" title="Next chunk">
            <AudioIcon name="forward" />
          </button>
          {showChunkMenuButton && (
            <button
              className={'audio-icon-btn audio-menu-btn' + (chunkMenuOpen ? ' audio-menu-btn-open' : '')}
              onClick={() => setChunkMenuOpen((value) => !value)}
              aria-label={chunkMenuOpen ? 'Hide audiobook chunk list' : 'Show audiobook chunk list'}
              aria-expanded={chunkMenuOpen}
              title="Jump to chunk"
            >
              <AudioIcon name="menu" />
            </button>
          )}
          <button className="audio-icon-btn" onClick={onStop} aria-label="Stop audiobook" title="Stop">
            <AudioIcon name="stop" />
          </button>
          {showPlaybackStatus && (
            <div className={'audio-floating-status tts-status-' + ttsState.status}>
              <span>{ttsState.status === 'error' ? ttsState.message : 'Chunk ' + (currentChunkNumber || 0) + '/' + chunkTotal}</span>
              {ttsState.status !== 'error' && playbackNotice && (
                <span>{playbackNotice}</span>
              )}
              {ttsState.status !== 'error' && ttsState.currentChunkDuration > 0 && (
                <span>{formatTtsTime(ttsState.currentChunkTime)} / {formatTtsTime(ttsState.currentChunkDuration)}</span>
              )}
              {ttsState.status !== 'error' && (
                <div className="tts-meter" aria-label={'Current chunk ' + chunkPercent + '% complete'}>
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
        <button className="audio-icon-btn" disabled aria-label="Preparing audiobook save" title="Preparing audiobook save">
          <AudioIcon name="save" />
        </button>
      )
    }

    if (isSaving) {
      return (
        <button
          className="audio-icon-btn"
          onClick={onManageSave}
          aria-label={'Audiobook save is ' + audiobookPercent + '%. Open Audiobooks to manage it.'}
          title={'Saving ' + audiobookPercent + '% - manage in Audiobooks'}
        >
          <span className="spinner audio-save-spinner" />
        </button>
      )
    }

    return (
      <button
        className={'audio-icon-btn' + (audiobookState.complete ? ' audio-save-complete' : '')}
        onClick={onSave}
        disabled={!canSaveAudiobook || audiobookState.complete}
        aria-label={audiobookState.complete ? 'Audiobook saved for this voice and speed' : 'Save audiobook'}
        title={audiobookState.complete ? 'Audiobook saved for this voice and speed' : 'Save audiobook'}
      >
        <AudioIcon name="save" />
      </button>
    )
  }
}

interface ChunkMenuProps {
  chunks: TtsChunkSummary[]
  currentChunkIndex: number | null
  chunksTotal: number
  playbackDurationSec?: number
  onSelect: (index: number) => void
}

const CHUNK_ROW_HEIGHT = 44
const CHUNK_MENU_VISIBLE_ROWS = 12
const CHUNK_MENU_OVERSCAN = 6

const ChunkMenu = memo(function ChunkMenu({
  chunks,
  currentChunkIndex,
  chunksTotal,
  playbackDurationSec,
  onSelect,
}: ChunkMenuProps) {
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
    <div className="audio-chunk-menu" aria-label="Audiobook chunk list">
      <div className="audio-chunk-menu-header">
        <span>Chapters</span>
        <span>{chunks.length} chunks</span>
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
                  <span className="audio-chunk-text">{chunk.textPreview}</span>
                  <span className="audio-chunk-number">{chunk.index + 1}</span>
                </button>
              )
            })}
          </div>
        </div>
      </div>
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
