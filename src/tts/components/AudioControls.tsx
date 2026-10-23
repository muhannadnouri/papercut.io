import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import {
  createPortal,
  flushSync,
} from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import type { TtsChunkSummary, TtsPlayerState } from '../hooks/useTtsPlayer'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { formatSpeedLabel } from '../utils/format'
import { SavedAudiobooksMenu } from './SavedAudiobooksMenu'
import './AudioControls.css'

interface AudioControlsProps {
  playAnywhere?: {
    browsing: boolean
    capturing?: boolean
    pending: boolean
    feedback: string | null
    onSnapshotSelection?: () => boolean
    onPlayFromSelection?: () => void
    onSelectionMenuClose?: () => void
    onPlayFromHere: () => void
    onPlayFromBeginning: () => void
    onReturnToAudio: () => void
  }
  audiobookState: AudiobookCacheState
  canPlayAudiobook: boolean
  canSaveAudiobook: boolean
  canSkipBackward: boolean
  canSkipForward: boolean
  modelSetupLabel: string | null
  saveInProgress: boolean
  onManageSave: () => void
  onPause: () => void
  onRead: () => void
  onResume: () => void
  onSelectSavedAudiobook: (record: SavedAudiobookRecord) => void
  onJumpToChunk: (index: number) => void
  onPlaybackRateChange: (rate: number) => void
  onOpenAudioSetup: () => void
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

type AudioIconName = 'play' | 'pause' | 'resume' | 'stop' | 'back' | 'forward' | 'save' | 'menu' | 'options' | 'passage' | 'selection' | 'beginning' | 'return'
const PLAYBACK_RATE_OPTIONS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3]

export function AudioControls({
  playAnywhere,
  audiobookState,
  canPlayAudiobook,
  canSaveAudiobook,
  canSkipBackward,
  canSkipForward,
  modelSetupLabel,
  saveInProgress,
  onManageSave,
  onPause,
  onRead,
  onResume,
  onSelectSavedAudiobook,
  onJumpToChunk,
  onPlaybackRateChange,
  onOpenAudioSetup,
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
  const idlePlayRef = useRef<HTMLButtonElement | null>(null)
  const restoreTransportFocusRef = useRef(false)
  const dockRef = useRef<HTMLDivElement | null>(null)
  const returnButtonRef = useRef<HTMLButtonElement | null>(null)
  const menuButtonRef = useRef<HTMLButtonElement | null>(null)
  const [portalTarget, setPortalTarget] = useState<Element | null>(null)
  const registerControls = useCallback((node: HTMLElement | null) => {
    controlsRef.current = node
    if (node) setPortalTarget(node.closest('.app-reader') ?? node.ownerDocument.body)
  }, [])
  const registerReturn = useCallback((node: HTMLButtonElement | null) => {
    if (!node && returnButtonRef.current === returnButtonRef.current?.ownerDocument.activeElement) restoreTransportFocusRef.current = true
    returnButtonRef.current = node
  }, [])
  const registerIdlePlay = useCallback((node: HTMLButtonElement | null) => {
    if (!node && idlePlayRef.current === idlePlayRef.current?.ownerDocument.activeElement) restoreTransportFocusRef.current = true
    idlePlayRef.current = node
  }, [])
  useLayoutEffect(() => {
    if (restoreTransportFocusRef.current) {
      dockRef.current?.querySelector<HTMLButtonElement>('.audio-primary-btn:not(:disabled), .audio-menu-btn')?.focus()
      restoreTransportFocusRef.current = false
    }
  })
  const [chunkMenuOpen, setChunkMenuOpen] = useState(false)
  const [selectionAvailable, setSelectionAvailable] = useState(false)
  useLayoutEffect(() => {
    const dock = dockRef.current
    const view = dock?.ownerDocument.defaultView
    if (!chunkMenuOpen || !dock || !view) return
    /** Large text can wrap the player into several rows; fit above its actual surface, not an assumed toolbar height. */
    const fit = () => {
      const gap = parseFloat(view.getComputedStyle(dock).fontSize) * 0.4 + 8
      const available = Math.max(0, dock.getBoundingClientRect().top - (view.visualViewport?.offsetTop ?? 0) - gap)
      dock.style.setProperty('--audio-menu-available-height', available + 'px')
    }
    fit()
    const resize = typeof view.ResizeObserver === 'function' ? new view.ResizeObserver(fit) : undefined
    resize?.observe(dock)
    view.addEventListener('resize', fit)
    view.visualViewport?.addEventListener('resize', fit)
    view.visualViewport?.addEventListener('scroll', fit)
    return () => {
      resize?.disconnect()
      view.removeEventListener('resize', fit)
      view.visualViewport?.removeEventListener('resize', fit)
      view.visualViewport?.removeEventListener('scroll', fit)
      dock.style.removeProperty('--audio-menu-available-height')
    }
  }, [chunkMenuOpen])
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
  const showPlaybackMenuButton = showFloatingPlayback || Boolean(playAnywhere && canPlayAudiobook)
  const showPlaybackStatus = ttsState.status !== 'idle'
  const playbackRateLabel = formatSpeedLabel(playbackRate)
  const noticePending = ttsState.status !== 'error' && Boolean(playAnywhere?.pending || ttsState.status === 'loading')
  const noticeError = ttsState.status === 'error' || Boolean(playAnywhere?.feedback)
  let notice = ''
  if (ttsState.status === 'error') {
    notice = ttsState.message
  } else if (noticePending) {
    notice = ttsState.status === 'loading'
      ? ttsState.message || t('tts.status.preparingPlayback')
      : t('tts.playAnywhere.preparing')
  } else if (playAnywhere?.feedback) {
    notice = t(playAnywhere.feedback === 'not-ready' ? 'tts.playAnywhere.notReady'
      : playAnywhere.feedback === 'no-visible-text' ? 'tts.playAnywhere.noText'
        : playAnywhere.feedback === 'no-selection' ? 'tts.playAnywhere.noSelection'
          : playAnywhere.feedback === 'stale' ? 'tts.playAnywhere.stale'
            : playAnywhere.feedback === 'failed' ? 'tts.playAnywhere.failed' : 'tts.playAnywhere.unmapped')
  }

  const snapshotSelection = () => setSelectionAvailable(playAnywhere?.onSnapshotSelection?.() ?? false)
  /** Viewport capture must see the closed layout; selection instead consumes its snapshot before menu cleanup. */
  const closeMenu = () => {
    flushSync(() => setChunkMenuOpen(false))
    menuButtonRef.current?.focus()
  }
  const toggleMenu = () => {
    if (!chunkMenuOpen) snapshotSelection()
    setChunkMenuOpen(value => !value)
  }
  const onSelectionMenuClose = playAnywhere?.onSelectionMenuClose
  useEffect(() => { if (chunkMenuOpen) return () => onSelectionMenuClose?.() }, [chunkMenuOpen, onSelectionMenuClose])

  const handleChunkSelect = useCallback((index: number) => {
    setChunkMenuOpen(false)
    menuButtonRef.current?.focus()
    onJumpToChunk(index)
  }, [onJumpToChunk])

  const handlePlaybackRateChange = useCallback(() => {
    onPlaybackRateChange(nextPlaybackRate(playbackRate))
  }, [onPlaybackRateChange, playbackRate])

  useEffect(() => {
    if (!chunkMenuOpen) return
    dockRef.current?.querySelector<HTMLButtonElement>('.audio-chunk-menu button:not(:disabled)')?.focus()

    function handlePointerDown(event: PointerEvent) {
      const root = controlsRef.current
      if (!root || root.contains(event.target as Node) || dockRef.current?.contains(event.target as Node)) return
      setChunkMenuOpen(false)
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') { setChunkMenuOpen(false); menuButtonRef.current?.focus() }
    }

    document.addEventListener('pointerdown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [chunkMenuOpen])

  return (
    <section ref={registerControls} className="audio-controls" aria-label={t('tts.controls.ariaLabel')}>
      <div className="audio-compact-row">
        {savedAudiobooks.length > 0 && (
          <SavedAudiobooksMenu
            records={savedAudiobooks}
            selectedId={selectedAudiobookId}
            onSelect={onSelectSavedAudiobook}
          />
        )}
        {!showFloatingPlayback && canPlayAudiobook && (
          <button ref={registerIdlePlay} className="audio-icon-btn audio-primary-btn" onClick={playAnywhere?.onPlayFromHere ?? onRead} disabled={playAnywhere?.pending && ttsState.status !== 'error'} aria-label={t(playAnywhere ? 'tts.playAnywhere.here' : 'tts.controls.playSaved')} title={t(playAnywhere ? 'tts.playAnywhere.description' : 'tts.controls.playSaved')}>
            {playAnywhere?.pending ? <span className="spinner audio-save-spinner" aria-hidden="true" /> : <AudioIcon name="play" />}
          </button>
        )}
        {!showFloatingPlayback && playAnywhere && canPlayAudiobook && (
          <button ref={menuButtonRef} className="audio-icon-btn" onPointerDown={() => { if (!chunkMenuOpen) snapshotSelection() }} onClick={toggleMenu}
            aria-label={chunkMenuOpen ? t('tts.controls.hideMenu') : t('tts.controls.showMenu')}
            aria-expanded={chunkMenuOpen} title={t('tts.controls.menuTitle')}><AudioIcon name="menu" /></button>
        )}
        {renderSaveButton()}
      </div>

      {portalTarget && createPortal(
        <div ref={dockRef} className="audio-playback-dock" role="group" aria-label={t('tts.controls.playbackControls')}>
          {showPlaybackMenuButton && chunkMenuOpen && (
            <ChunkMenu
              onPlayFromHere={playAnywhere && canPlayAudiobook ? () => {
                closeMenu()
                playAnywhere.onPlayFromHere()
              } : undefined}
              onReturnToAudio={playAnywhere && canPlayAudiobook && playAnywhere.browsing ? () => {
                closeMenu()
                playAnywhere.onReturnToAudio()
              } : undefined}
              targetPending={Boolean(playAnywhere?.pending && ttsState.status !== 'error')}
              selectionAvailable={selectionAvailable}
              // Consume the saved selection before the menu's cleanup releases it.
              onPlayFromSelection={playAnywhere?.onPlayFromSelection && canPlayAudiobook ? () => {
                setChunkMenuOpen(false)
                menuButtonRef.current?.focus()
                playAnywhere.onPlayFromSelection?.()
              } : undefined}
              onPlayFromBeginning={playAnywhere && canPlayAudiobook ? () => {
                closeMenu()
                playAnywhere.onPlayFromBeginning()
              } : undefined}
              chunks={showFloatingPlayback ? ttsState.chunkSummaries : []}
              currentChunkIndex={ttsState.currentChunkIndex}
              chunksTotal={ttsState.chunksTotal}
              playbackDurationSec={playbackDurationSec}
              playbackRateLabel={playbackRateLabel}
              onPlaybackRateChange={handlePlaybackRateChange}
              wordHighlightEnabled={wordHighlightEnabled}
              onSelect={handleChunkSelect}
              onWordHighlightEnabledChange={onWordHighlightEnabledChange}
            />
          )}

          <div className="audio-playback-surface">
            <div className={'audio-target-notice' + (noticePending ? ' audio-target-pending' : noticeError ? ' audio-target-error' : '')
              + (playAnywhere?.capturing || chunkMenuOpen ? ' audio-target-capturing' : '')}
              role="status" aria-live="polite" aria-atomic="true" dir="auto">
              {notice && <>
                {noticePending && <span className="spinner" aria-hidden="true" />}
                <span>{notice}</span>
              </>}
            </div>

            {showFloatingPlayback && (
              <div className="audio-floating-playback" dir="ltr" aria-label={t('tts.controls.playbackControls')}>
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
                    ref={menuButtonRef}
                    className={'audio-icon-btn audio-menu-btn' + (chunkMenuOpen ? ' audio-menu-btn-open' : '')}
                    onPointerDown={() => { if (!chunkMenuOpen) snapshotSelection() }}
                    onClick={toggleMenu}
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
                {playAnywhere && canPlayAudiobook && playAnywhere.browsing && (
                  <button ref={registerReturn} type="button" className="audio-icon-btn audio-return-control" dir="auto"
                    aria-label={t('tts.playAnywhere.return')} title={t('tts.playAnywhere.return')}
                    onClick={playAnywhere.onReturnToAudio}>
                    <AudioIcon name="return" />
                    <span className="audio-return-label">{t('tts.playAnywhere.returnShort')}</span>
                  </button>
                )}
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
          </div>
        </div>, portalTarget,
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
    const buttonLabel = audiobookState.complete ? t('tts.controls.savedForCurrentSetup') : saveLabel
    const modelSetupRequired = modelSetupLabel !== null

    return (
      <button
        className={'audio-icon-btn' +
          (audiobookState.complete ? ' audio-save-complete' : '') +
          (modelSetupRequired ? ' audio-setup-required' : '')}
        onClick={modelSetupRequired ? onOpenAudioSetup : onSave}
        disabled={!modelSetupRequired && (!canSaveAudiobook || audiobookState.complete)}
        aria-label={modelSetupLabel ?? buttonLabel}
        title={modelSetupLabel ?? buttonLabel}
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
  onPlayFromHere?: () => void
  targetPending: boolean
  selectionAvailable: boolean
  onPlayFromSelection?: () => void
  onPlayFromBeginning?: () => void
  onReturnToAudio?: () => void
  chunks: TtsChunkSummary[]
  currentChunkIndex: number | null
  chunksTotal: number
  playbackDurationSec?: number
  playbackRateLabel: string
  onPlaybackRateChange: () => void
  wordHighlightEnabled: boolean
  onSelect: (index: number) => void
  onWordHighlightEnabledChange: (enabled: boolean) => void
}

const CHUNK_ROW_HEIGHT = 44
const CHUNK_MENU_VISIBLE_ROWS = 12
const CHUNK_MENU_OVERSCAN = 6

const ChunkMenu = memo(function ChunkMenu({
  onPlayFromHere,
  targetPending,
  selectionAvailable,
  onPlayFromSelection,
  onPlayFromBeginning,
  onReturnToAudio,
  chunks,
  currentChunkIndex,
  chunksTotal,
  playbackDurationSec,
  playbackRateLabel,
  onPlaybackRateChange,
  wordHighlightEnabled,
  onSelect,
  onWordHighlightEnabledChange,
}: ChunkMenuProps) {
  const { t } = useTranslation()
  const selectionHintId = useId()
  const viewId = useId()
  // Opening during playback prioritizes navigation; idle audio has no loaded list yet.
  const [view, setView] = useState<'chunks' | 'options'>(() => chunks.length ? 'chunks' : 'options')
  const showChunks = chunks.length > 0 && view === 'chunks'
  const listRef = useRef<HTMLDivElement | null>(null)
  const listBrowsing = useRef(false)
  const listPosition = useRef(0)
  /** Programmatic scroll events also fire; only reader gestures/focus suspend automatic centering. */
  const browseList = () => {
    listBrowsing.current = true
  }
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
    if (listBrowsing.current) {
      list.scrollTop = listPosition.current
      return
    }
    const nextScrollTop = Math.max(
      0,
      currentChunkIndex * CHUNK_ROW_HEIGHT - (list.clientHeight - CHUNK_ROW_HEIGHT) / 2,
    )
    list.scrollTop = nextScrollTop
    listPosition.current = nextScrollTop
    setScrollTop(nextScrollTop)
  }, [currentChunkIndex, showChunks])

  return (
    <div className="audio-chunk-menu" aria-label={t('tts.controls.menuTitle')}>
      {chunks.length > 0 && (
        <div className="audio-menu-views" role="group" aria-label={t('tts.controls.menuTitle')}>
          <button id={viewId + '-chunks'} type="button" aria-pressed={showChunks}
            onClick={() => setView('chunks')}>
            <AudioIcon name="menu" />
            <span>{t('tts.controls.chunkView')} · {chunks.length.toLocaleString()}</span>
          </button>
          <button id={viewId + '-options'} type="button" aria-pressed={!showChunks}
            onClick={() => setView('options')}>
            <AudioIcon name="options" />
            <span>{t('tts.controls.optionsView')}</span>
          </button>
        </div>
      )}
      {!showChunks && onPlayFromHere && (
        <div className="audio-passage-actions">
          <button
            type="button"
            className="audio-passage-command audio-passage-here"
            onClick={onPlayFromHere}
            disabled={targetPending}
            title={t('tts.playAnywhere.description')}
          >
            <AudioIcon name="passage" />
            <span>{t('tts.playAnywhere.here')}</span>
          </button>
          {onPlayFromSelection && (
            <button
              type="button"
              className="audio-passage-command"
              onClick={onPlayFromSelection}
              disabled={!selectionAvailable || targetPending}
              aria-describedby={!selectionAvailable ? selectionHintId : undefined}
            >
              <AudioIcon name="selection" />
              <span>{t('tts.playAnywhere.selection')}</span>
            </button>
          )}
          {onReturnToAudio && (
            <button type="button" className="audio-passage-command" onClick={onReturnToAudio}>
              <AudioIcon name="return" />
              <span>{t('tts.playAnywhere.return')}</span>
            </button>
          )}
          {onPlayFromBeginning && (
            <button type="button" className="audio-passage-command" onClick={onPlayFromBeginning}>
              <AudioIcon name="beginning" />
              <span>{t('tts.playAnywhere.beginning')}</span>
            </button>
          )}
          {onPlayFromSelection && !selectionAvailable && (
            <p id={selectionHintId} className="audio-selection-hint">{t('tts.playAnywhere.noSelection')}</p>
          )}
        </div>
      )}
      {!showChunks && <div className="audio-chunk-menu-header">
        <span>{t('tts.controls.playback')}</span>
        <div className="audio-playback-settings" role="group" aria-label={t('tts.controls.playback')}>
          <button
            type="button"
            className="audio-word-highlight-toggle"
            onClick={() => onWordHighlightEnabledChange(!wordHighlightEnabled)}
            aria-pressed={wordHighlightEnabled}
            aria-label={t('tts.controls.wordHighlightState', {
              state: wordHighlightEnabled ? t('tts.setup.on') : t('tts.setup.off'),
            })}
          >
            <span>{t('tts.controls.wordHighlight')} <span className="audio-beta-tag">{t('tts.controls.experimental')}</span></span>
            <span>{wordHighlightEnabled ? t('tts.setup.on') : t('tts.setup.off')}</span>
          </button>
          <button type="button" className="audio-menu-rate"
            onClick={onPlaybackRateChange}
            title={t('tts.controls.playbackSpeedTitle')}
            aria-label={t('tts.controls.playbackSpeed', { speed: playbackRateLabel })}>
            {playbackRateLabel}
          </button>
        </div>
      </div>}
      {showChunks && (
          <div
            ref={listRef}
            aria-labelledby={viewId + '-chunks'}
            className="audio-chunk-list"
            onPointerDown={browseList}
            onWheel={browseList}
            onFocusCapture={browseList}
            onScroll={(event) => {
              listPosition.current = event.currentTarget.scrollTop
              setScrollTop(event.currentTarget.scrollTop)
            }}
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
    case 'passage':
      return (
        <>
          <path d="M3 5h18M3 10h9M3 15h7M3 20h7" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" />
          <path d="m15 12 7 5-7 5z" />
        </>
      )
    case 'selection':
      return (
        <>
          <path d="M3 9V4h5m8 0h5v5M3 15v5h5m13-5v5h-5M7 8h10M7 12h6M7 16h6" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" />
          <path d="m16 10 7 5-7 5z" />
        </>
      )
    case 'beginning':
      return (
        <>
          <path d="M5 4v16" fill="none" stroke="currentcolor" strokeWidth="2" />
          <path d="m19 5-11 7 11 7z" />
        </>
      )
    case 'return':
      return (
        <>
          <path d="M3 8h10a7 7 0 0 1 0 14M3 8l5-5M3 8l5 5" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          <path d="M12 13h5m-5 4h4" fill="none" stroke="currentcolor" strokeWidth="2" />
        </>
      )
    case 'save':
      return <path d="M5 3h12l2 2v16H5zM8 3v6h8V3M8 18h8v-5H8z" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinejoin="round" />
    case 'menu':
      return <path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" />
    case 'options':
      return <path d="M4 6h3m4 0h9M4 12h9m4 0h3M4 18h3m4 0h9M7 3v6m6 0v6m-6 0v6" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" />
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
