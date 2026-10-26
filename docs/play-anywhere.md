# Play anywhere: integration plan

Status: source capture, verified prepared-chunk lookup, guarded player integration
and viewport/selection controls with follow behavior are implemented through
Stage 5. Stopped play captures the visible passage; the audio menu also offers
Play from selection. Resume retains its exact position and Play from beginning
is explicit. Stage 6 Linux AppImage acceptance is in progress; owner-reported
false passage-change, imported punctuation-spacing and sentence-closer rejections
have automated regressions and pending manual retests. Baseline listening, device,
assistive-technology and performance gates remain open. Stage 8 (cancellation
consolidation, see §3 “Cancellation model”) is implemented, unit-verified and
smoke-tested on desktop and Android; iOS waits for TestFlight.
Created 2026-10-21 against `main` at `8ebc9d0f`; working branch:
`feature/play-anywhere`. Checked boxes record only the specific completed work.
Feature, performance, accessibility, and device acceptance remain pending.

## 1. Decision and scope

Let a user scroll to a passage in a completed saved audiobook and play from the
**beginning of the existing saved chunk containing that passage**, continuing
through the rest of the book. This chunk-start behavior is the explicit product
decision. Do not estimate an intra-chunk offset or regenerate narration.

- [x] Agree on chunk-start playback rather than word-level seeking.
- [x] Inspect the current reader, chunk metadata, desktop/mobile playback,
  highlighting, imported-bundle compatibility, and diagnostics boundaries.
- [x] Create and switch from `main` to `feature/play-anywhere`.
- [ ] Complete the implementation stages and acceptance gates below before
  describing the feature as available.

In scope: bundled HTML and imported reflowable HTML, including converted EPUB,
TXT, and Markdown; embedded-text and recognized-text PDFs where a verified text
mapping exists; completed local and imported saved audiobooks; desktop, Android,
and iOS playback. Selection targeting is a second delivery stage using the same
resolver. Raw EPUB rendering is not required: uploaded EPUBs use generated HTML.

Out of scope: word timestamps, forced alignment, synthesis, new chunk sizes,
partial-download playback, new audiobook formats, persistent listening history,
cloud sync, gestures that make every text tap play, a replacement reader/player,
new dependencies, a general plugin framework, and unrelated translation work.
No storage migration, cache epoch change, or model download is planned.

## 2. UX contract

### Controls and playback state

| Context/action | Required behavior |
| --- | --- |
| Completed audiobook, stopped | Reader primary play is **Play from here**. Capture the visible passage and start its containing chunk at offset zero. |
| Paused | Primary action is **Resume**, preserving the paused audio position regardless of scrolling. |
| Playing | Primary action is Pause. Browsing never seeks or pauses audio on its own. |
| User browses during playing or paused playback | Suspend automatic following. Show **Return to audio** inside the transport surface; **Play from here** is in its popover. Retain normal transport controls and meanings; no separate sticky browse panel. |
| Play from here while paused | Seek to the chosen chunk beginning and play; a seek that remains paused does not satisfy this action. |
| Return to audio | Reveal the current audio passage and enable following. Preserve playing/paused state and current audio time. |
| Play from beginning | Explicit audio-menu action; start canonical chunk zero. Also provides an accessible fallback when location mapping is unavailable. |
| Explicit selection action | **Play from selection** in the audio menu targets the selection's start in document order, then plays its containing chunk. |
| Stop | Retain current stop semantics and end the browse/follow session. A later Play from here captures the then-current viewport. |
| Missing/incomplete saved audio | Preserve current save/setup affordances. This feature does not generate missing audio. |

Use the existing button, menu, icon, focus, and responsive styling patterns.
The primary play button needs an accessible name and tooltip. The idle floating
shortcut stays icon-only inside the compact reader-tools capsule, including after
Stop; its accessible name still identifies the viewport/selection action. Labeled
passage commands remain in Options. “Chunk” is unnecessary
in primary UI copy. Supporting description: “Starts at the beginning of the audio
segment containing this passage.” Keep selection, hyperlinks, and current
fullscreen toolbar gestures working normally. No new global shortcut is needed.
The external playback disclosure uses icon-only sliders, matching the Options tab;
its full accessible name and tooltip are Playback options. Reader tools and player share a bottom edge when measured
widths fit; otherwise the capsule stacks above the player at the same end edge.
Existing saved audio remains playable when setup for new generation is missing;
Save retains the floppy disk: green tonal when saved for the current ready setup,
soft yellow with a warning marker when generation setup is needed.

### Which passage counts as “here”

1. Viewport actions use the **first visible narratable text in reading order**
   within the unobscured reader area. Exclude the fixed header, PDF toolbar,
   floating player, clipped regions, hidden content, and non-narrated references.
   Do not use the window bounds for a PDF's nested scroll container.
2. If the top visible line cuts through a paragraph or a chunk, use a text point
   on that visible line, not the paragraph's potentially distant beginning. Map
   that point to the containing saved chunk and start the whole chunk.
3. For multiple columns and RTL, use the existing narration/source order to
   choose among visible candidates. Screen x/y order alone is insufficient.
4. Images and margins are skipped in favor of narratable text still in view.
   If none exists, return unavailable; do not choose an arbitrary offscreen chunk.
5. A selection action uses the normalized Range start, including backward
   selections. Both endpoints must belong to the same current reader document.
   Snapshot the selection before menu focus collapses it. An ordinary viewport
   action ignores incidental text selection.
6. Whitespace-only points advance to the next mapped narrated character within
   the visible passage; an excluded footnote marker must not select unrelated
   footnote audio. If there is no trustworthy mapping, return unavailable.
7. Once playback successfully starts, highlight its actual chunk and reveal its
   beginning if needed. This can move slightly above the previous viewport.
   Following may resume after this explicit action. Failed resolution does not
   move the view or interrupt existing audio.

### Failure and feedback

Use localized, short notices for mapping still preparing, no readable text in
view, unsupported text mapping, stale target, and saved-audio failure. A missing
mapping must never silently play chunk zero. Offer Play from beginning and the
existing chunk menu when applicable; never make a fallback seek without an
explicit action. Keep the current audio session intact until resolution succeeds;
if loading/seeking then fails, report the actual player error without promising
restoration of the old position. Reuse existing loading/status UI; no invented
progress percentages. No audio starts merely because a document opens.

### Following versus browsing

Keep one session-local `following`/`browsing` state near the reader, independent
of the player's playing/paused state. User wheel/touch-scroll/keyboard navigation,
scrollbar interaction, and explicit Find/bookmark/link jumps suspend following.
Selection that moves the view also suspends following. Programmatic reveal for
audio must not mark itself as user browsing: do not classify every scroll event
as user intent. Define and test the smallest reliable input/programmatic-scroll
distinction for both window and PDF containers, including mobile momentum.

While browsing, continue updating highlights without scrolling **or changing the
PDF current page** to the playing passage. Cancel already scheduled automatic
scrolls when browsing begins. Return to audio and successful Play from here
restore following. A newer browse input overrides a pending reveal. Respect
reduced motion. Closing/replacing the document or stopping clears this transient
state and all related listeners/timers.

## 3. Architecture and separation of concerns

```text
AudioControls: explicit user intent
    -> viewer adapter: capture a current text point
    -> play-anywhere coordinator: bind document/audiobook/request identity
    -> format mapper + pure resolver: point -> canonical chunk index
    -> existing player: initial start or active seek-and-play
    -> existing highlight renderer + independent follow policy
```

### Ownership boundaries

| Owner | Responsibility | Must not own |
| --- | --- | --- |
| HTML/PDF viewer adapter | Capture viewport/selection points, reader bounds, source order, live-view revision; reveal current passage. | Audio loading, saved-audiobook selection, synthesis, player state. |
| Feature-owned code under `src/tts/playAnywhere/` | HTML/PDF lookup, capture and verified source preparation are implemented; source-to-player coordination and reader follow/browse intent are implemented. Keep only files that earn their existence. | Native audio session, new persistent settings, parsing/import pipelines. |
| `useAudiobookDocument` / alignment helpers | Authoritative prepared chunks and validated/recovered source spans; common normalization primitives. | Control rendering and playback decisions. |
| `useAudiobookManager` | Bind selected saved audiobook/options/prepared chunks to the feature and expose actions to controls. | DOM inspection or another seek queue. |
| `useTtsPlayer` | Validate starting index, load/seek/play, cancellation, queue ordering, playback errors, desktop/native state. | Viewport, selection, PDF DOM, reading-order heuristics. |
| Highlight renderers | Render the actual active chunk, including while browsing. Apply an optional follow policy. | Choosing what plays or automatically invoking Play from here. |
| `App` / `DocumentViewer` | Pass optional capabilities/actions through existing composition and viewer API registration. | Duplicated mapping or audio logic. |

Use explicit typed inputs/results, ordinary functions and existing hooks. The
pure resolver must run without React, DOM, native APIs, or global state. Format
adapters may inspect DOM/PDF.js. Use type-only imports for boundary contracts;
runtime imports must flow in one direction without cycles. Do not broaden a
bookmark API into an audio service or create a second playback controller.

### Boundary contracts

Contracts with implementation receipts below exist. Device/assistive-technology
and performance acceptance remains pending.

| Contract | Input/output and lifetime |
| --- | --- |
| Optional reader target API | `onTextTargetApiChange` registers/unregisters `ViewerTextTargetApi`. Its implemented `captureViewport()` returns a Promise of a captured HTML or PDF point or typed unavailable reason. Optional synchronous `captureSelection()` returns the same result type from the normalized Range start; absent capabilities remain compatible. No audio side effects. |
| Target snapshot | HTML capture snapshots viewport geometry synchronously, then yields at least two animation frames for browser visibility delivery (up to 500 ms for initial observations). Source/view mutations, scroll or reflow before completion return stale rather than retargeting. HTML output includes frozen normalized `segmentTexts` and `segmentLengths` arrays reused for the lifetime of its source index; PDF includes the original item/word `sourceText`. Both provide `isCurrent()` for source/view lifetime validation. PDF captures already-rendered geometry synchronously inside the Promise method, with no animation-frame wait or page rendering. No DOM references are serialized. Selection snapshots its actual DOM point before menu focus loss. Both endpoints must belong to the current reader. The snapshot survives native collapse during Tab/button focus changes, but rejects a different live range; a reader input epoch invalidates pending targets on fresh non-audio pointer input or non-Tab key input. |
| Normalized source point | HTML: readable segment index + normalized UTF-16 offset. PDF: page index + original text-item/block order + source offset. Do not conflate the two coordinate systems. |
| Resolver result | `resolved` with canonical chunk index/id, or `unavailable` with `not-ready`, `unmapped`, `ambiguous`, `invalid-source` or `invalid-point`. Capture/coordinator additionally own `no-visible-text`, `no-selection` and `stale`; the pure resolver does not inspect DOM or session identity. Unexpected errors remain errors rather than successful fallbacks. |
| Prepared target boundary | `useAudiobookManager.preparedChunks` exposes canonical chunks without active playback. `resolveTextTarget(documentUrl, capture, signal?)` returns a Promise of a verified canonical index/ID plus original full chunks, document URL and `isCurrent()`, or typed unavailable. It never starts audio. Commit-time document/content/model/chunk changes and unmount dispose preparation caches. The Stage 3 coordinator additionally guards action/options changes and synchronous cancellation before dispatch. |
| Player initial start | `speak(chunks, options, startIndex = 0)` validates a safe integer in the canonical nonempty sequence before changing audio. Pass the complete sequence, never a sliced tail. It returns whether dispatch was accepted, not whether sound was heard. |
| Player active target | `playFromChunk(fullChunks, options, index)` returns `Promise<PlaybackStart> \| null`: `null` rejects before any audio change; the Promise settles once with `started` (Play acknowledged), `superseded` (a later command) or `failed` (player error). It reuses an active session only for matching canonical IDs/text/hashes and options. It uses `jumpToChunk(index, true)` for explicit seek-and-play, otherwise starts the existing player at that index. Ordinary mobile jump keeps paused state; Resume keeps time on both platforms. Desktop ordinary jump retains its prior play behavior. |
| Target playback action | `usePlayAnywherePlayback` owns newest action identity until dispatch, within an immutable audio-option/document scope, and follows the player's `commandSignal()`. Manager exposes `playFromHere(viewerApi)`; existing Read uses `playFromBeginning()`. Results are `dispatched { started }`, `cancelled { superseded \| disposed }`, typed `unavailable`, or `error` with the original error. The reader hook presents loading/failure results and never reports `cancelled`. `started` follows the transport Play Promise; it is not acoustic confirmation. |
| Follow policy | Optional `AudioFollowPolicy { allowsReveal(), revision, active? }` preserves automatic following when omitted. `allowsReveal()` reads synchronous reader intent at actual reveal execution; revision makes Return reveal even the same paused chunk; active scopes input listener lifetime. HTML timers and PDF page/range reveals use it; highlighting remains independent. |

At the coordinator boundary bind each request to selected document, current view
revision, selected audiobook identity/options, chunk sequence revision, and a
monotonic action id. Reuse existing identities/references where sufficient; do
not hash the entire book per click. Recheck after each await and immediately
before the player side effect. Any player command (Stop, Pause, skip, jump, a
newer start) and any scope change (model/voice/options, audiobook readiness,
document) or unmount cancel pending requests without wrappers. A late completion
cannot restart stopped audio, play a different book, update a replacement view, or
override a newer target. Player generation guards alone are authoritative after
dispatch.

### Cancellation model

Three concerns, each with one owner. Why the model changed, and what changed in
behavior, is recorded in the 2026-10-26 review and Stage 8 implementation
entries in §6.

| Concern | Question | Single owner | Valid until |
| --- | --- | --- | --- |
| Intent | Is this still the newest playback command? | `useTtsPlayer` command epoch | The next public player command |
| Validity | Does the resolved chunk still describe the passage the user pointed at? | Viewer capture `isCurrent()` and preparation's document/content/model/chunk revision | Dispatch |
| Lifetime | Does the owning scope still exist? | Each hook's `dispose()` (reader, playback scope, preparation) | Unmount or dependency change |

1. **One intent source.** `useTtsPlayer` replaces an `AbortController` (the
   command epoch) at the top of each public command: `speak`, `playFromChunk`,
   `pause`, `resume`, `skipByChunks`, `jumpToChunk` and `stop`. Automatic
   advance, preload, cache pruning and native state sync keep the epoch.
   `commandSignal()` returns the current signal. Nothing outside the player
   cancels targets for transport or settings. Unmount replaces the epoch rather
   than only aborting it, so React StrictMode's effect replay leaves a live one.
2. **The target scope follows the epoch.** `createPlayAnywherePlayback` aborts
   its previous run and forwards the epoch's abort into the new run's
   `AbortController` with a once-listener, removed when the run settles. It
   does not use `AbortSignal.any` or `signal.reason` (iOS minimum 15.0). That
   signal feeds capture, `resolve(url, capture, signal)` and preparation.
   Option, document, readiness and source changes recreate the scope, and
   `dispose()` cancels it.
3. **Validity ends at dispatch.** It is checked after every await and once
   immediately before the player call, never afterwards. After dispatch, only
   the player's job, epoch and attempt ids fence the start. Capture-time
   hidden/stale/input-epoch checks still apply before dispatch.
4. **Start ticket.** `playFromChunk(fullChunks, options, index)` returns
   `Promise<PlaybackStart> | null`. `null` rejects before any audio changes.
   The Promise settles once:
   - `started` on the Play acknowledgement;
   - `superseded` when its epoch aborts;
   - `failed` from the single `status === 'error'` transition.

   The player takes no external guards. `isCurrentJob(jobId)` is a pure
   comparison that the native pre-mutation predicates use.
5. **Typed outcomes.** `PlaybackTargetResult` is one of:
   - `dispatched { started }`;
   - `cancelled { reason: 'superseded' | 'disposed' }`;
   - `unavailable { reason }`;
   - `error { error }`.

   `stale` means only that validity failed. `cancelled` never produces
   feedback.
6. **The reader only presents.** `usePlayAnywhereReader`:
   - keeps a local run counter only to ignore its own older runs;
   - clears `pending` when the result or ticket settles, and restores following
     on `started` if no newer browse happened;
   - clears feedback through a once-listener on the next epoch.

   Selection snapshots belong to the selection/input epoch, so transport
   commands do not discard them.
7. **Other owners.** These own their own state:
   - the preparation scheduler and request coalescing;
   - viewer capture epochs;
   - native lifecycle serialization;
   - readiness/save/delete.

   The startup navigation UI gate is control-state only, because a relative
   skip needs a known index. A skip that reaches a starting target navigates.
   A dispatched start continues across a document switch, as ordinary playback
   does. Stopping audio on switch would be a product change for all playback.

### HTML and converted EPUB mapping

Reuse `htmlStructure`, `domTextSegments`, and chunker normalization so extraction,
highlighting, and reverse mapping share segment ownership, whitespace, skipped
references, and offset semantics. Add only the missing inverse point conversion
as a focused helper. Raw bookmark `textOffset` counts DOM text and is **not** a
normalized audio offset. Do not compare it directly with `sourceSpan` offsets.

Build an ordered chunk-span lookup for a stable sequence. Containment uses
half-open source ranges `[start, end)`; a character exactly at the next chunk's
start belongs to that next chunk. Selection at an element boundary must first
resolve to a narrated text point. End-of-document/invalid offsets must produce
an explicit result, not an accidental out-of-bounds index. Validate ordering and
bounds before binary search. Normalize nonempty chunks once so filtering cannot
shift playback indices. One source segment can contain multiple audio chunks.

Use durable source spans first and validate against the current readable text.
For old imported bundles, reuse the current lazy span reconstruction and ordered,
exact-first/Arabic-aware text fallback. Keep imported chunk text/order/id/audio
authoritative. Recovered spans are rebuildable runtime metadata. Repeated phrases
must preserve occurrence order; reject an unresolved ambiguity. Do not duplicate
normalization/fallback code or re-chunk imported audio to make it fit.

### PDF and OCR mapping

Use the currently visible PDF.js text layers and the same text-item mapping used
by `pdfTtsHighlight`, then look up chunk `pdfSourceSpans` by page/item/offset.
Retain canonical narration order across columns. Handle Find splitting text DOM,
zoom/rotation, ligatures and non-linear source runs; multiple chunk candidates
for an indivisible glyph need a deterministic source-order policy and a regression
test (choose the earliest overlapping canonical chunk). Never derive a chunk
from page percentage or the order of incidental DOM children.

PDF virtualizes pages. Capture from visible rendered layers only; do not render
the whole book for this action. Missing layers return not-ready. Recognized text
must map through the existing OCR layer/source run contract and receive its own
fixture/device acceptance. A readable image-only page without verified text
mapping remains unavailable. Do not silently treat it as the page's first audio
chunk. A future page-start action would require separate copy and scope approval.

Stage 0 audit found that OCR currently joins persisted word blocks into selectable
line spans (`pdfOcrTextLines`), exposing line text/bounds but no inverse word-block
offset map. Native PDF.js `_textHighlighter` mappings do not cover that overlay.
Stage 2b now retains a transient line-offset -> original block/order/offset map
through that same line builder. Its WeakMap belongs to rendered line elements;
inserted spaces and trimmed line ends are unmapped, while removed newlines retain
original UTF-16 offsets. Rendering records the map alongside each line; no observer
or target index is created without the optional capability consumer. No stored
OCR schema changes. Chromium overlay capture is verified below; Stage 4 also gates OCR audio-follow reveals and refreshes rendered-layer highlights.
Native WebView/device acceptance remains pending.

### Playback paths and compatibility

- Desktop: enter existing navigation/loading at the chosen index, play its WAV
  from `currentTime = 0`, then use existing bounded prefetch/cache pruning.
- Mobile initial start: prepare/reuse the existing seekable track, set source and
  rate, await seek to the validated chunk's actual `startSec`, then play. Guard
  every async transition. Do not audibly play zero and jump afterward.
- Mobile active session: serialize seek and play with existing lifecycle helpers;
  rapid target changes must not play a superseded location. Preserve source/session
  reuse and background/lock-screen ownership. Do not add a parallel command queue.
- Initial navigation gate: Previous/Next and chunk-list rows stay disabled while
  `currentChunkIndex` is null. Native startup must not publish its provisional seek
  reply as a paused/current chunk before Play acknowledgement. Stop and Options
  remain available; subsequent seeks keep the acknowledged index and navigation.
  This is a general control-state rule, independent of optional passage wiring;
  retain it if removing the feature. Since Stage 8 the gate is UI semantics only:
  a startup that a skip reaches navigates instead of idling.
- Never use the chunk menu's estimated timestamps for seeking. Chunk boundary
  metadata and the native source signature remain authoritative.
- Validate a supplied start index as a finite integer within the canonical
  sequence. Only an omitted index defaults to zero; invalid explicit inputs fail
  before replacing audio. Do not turn malformed inputs into clamped guesses.
- Preserve native-only saved-audio availability and browser preview limitations.
  Surface browser autoplay/permission rejection through existing player errors;
  do not add a silent browser synthesis fallback or start audio without intent.
- Preserve source bytes, chunk ids/text/order/spans, generation settings, manifest
  and bundle versions, cache identities, native commands and transfer format.
  No new Rust command is expected. If source inspection proves one necessary,
  amend this design and its compatibility/removal scope before adding it.

### Explicit integration and removal map

| Integration surface | Planned change | Removal action |
| --- | --- | --- |
| `src/tts/playAnywhere/` | `sourceChunkResolver.ts`, `pdfChunkSourceResolver.ts`, HTML/PDF capture, shared geometry, `verifiedSource.ts`, `prepareTextTarget.ts`, `preparationWork.ts`, `usePreparedTextTarget.ts` and tests exist; `usePlayAnywherePlayback.ts` coordinates playback; `usePlayAnywhereReader.ts` owns reader intent/feedback and `readerBrowse.ts` observes input; `selectionTarget.ts` validates selection/DOM boundaries and owns pre-focus intent listeners. | Disconnect App reader hook, viewer factories/viewport helpers and manager feature hooks/exports; dispose preparation to abort its signal and clear its timer, then delete feature-owned files. No stored state is owned here. |
| `src/viewers/types.ts`, `HtmlViewer.tsx`, `PdfViewer.tsx` | HTML/PDF viewport/selection result/API types and optional `onTextTargetApiChange` / `getTextTargetViewport` props exist. Both viewers register capture; optional `AudioFollowPolicy` and `onReaderBrowse` gate reveals/report explicit jumps. | Remove optional props, both registration effects and type imports; retain normal viewer behavior. |
| `src/components/DocumentViewer/DocumentViewer.tsx`, `src/App.tsx` | DocumentViewer forwards optional registration and supplies chrome-clipped bounds. App owns the optional reader hook, passes capture/follow/browse capabilities and binds control actions. DocumentViewer reports Find/bookmark/hash/top jumps and binds optional `onSelectionIntent` pre-focus/invalidation callbacks. | Remove the App hook and optional capture/follow/browse/selection-intent wiring; App no longer overrides transport actions. Restore manager onRead for idle play. Ordinary controls/bookmarks/Find still work. |
| `src/tts/hooks/useAudiobookManager.ts`, `useAudiobookDocument.ts` | Expose `preparedChunks` / `resolveTextTarget` / `playFromHere` / `playFromBeginning` / `commandSignal`; pass the player's epoch to the playback scope (no transport/options wrappers) and stamp prepared state by document/content/model and exclude unverified import display fallback. Canonical getter retries manifest loading. | Disconnect the two feature hooks/exports and restore zero-start Read wiring. Remove canonical/getter exports only if unused; ordinary Read now uses canonical preparation too. Keep document/model ownership guards, ordinary preparation, and highlight recovery. |
| `src/tts/hooks/useTtsPlayer.ts` | Optional start index, explicit seek-and-play, full sequence/option reuse, and a general command epoch (`commandSignal()`) with `PlaybackStart` tickets. No external guard inputs. | Restore zero-start calls and remove unused feature entry points; the epoch/ticket are general player API and may stay. Retain late-reply, pause/resume and teardown fixes used by ordinary playback. |
| `src/tts/playback/nativeMobileAudio.ts`, `playbackState.ts` | Optional predicates run immediately before queued native mutations; pure timing/option checks use native manifest tolerance. | Remove feature-only predicate inputs if unused; retain lifecycle serialization, timing compatibility and general cancellation protections. |
| `src/tts/alignment/domTextSegments.ts` and imported-span helpers | `normalizeDomTextPoint` supplies inverse offsets. `recoverOrderedSourceSpans` serves highlighting and target validation; normalized locator construction and sentence/clause compatibility are shared and its tolerant index is lazy. `locatorRangeIsIgnorable` proves context-dependent gaps for verified target preparation. | Remove inverse/snapshot-only helpers and `locatorRangeIsIgnorable` if unused after deleting feature validation; keep shared ordered recovery, punctuation compatibility and UTF-16 fixes used by highlighting. No shared alignment module imports play-anywhere. |
| `src/tts/alignment/narrationText.ts`, `src/tts/utils/text.ts` | Pure sentence/clause grammar shared by canonical generation and matching; Unicode projection retains original UTF-16 offsets. Shared generators have synchronous baseline wrappers; no timers or feature imports. | Retain the common grammar and synchronous generation/highlighting APIs. Delete only unused feature prewarming entry points; baseline callers require no feature scheduler. |
| `src/pdf/ocr/pdfOcrTextLayer.ts` | Render-time WeakMap and source runs invert selectable OCR lines to stored words. | Remove `sourceRuns`, `lineSources`, `pdfOcrSourcePoint` and its renderer assignment; restore direct line text concatenation and retain rendering/persisted OCR data. |
| `src/pdf/pdfTtsHighlight.ts` | Export existing pinned PDF item-map types for the capture adapter; highlight behavior is unchanged. | Remove the type exports if unused after removing capture. |
| `src/tts/hooks/useTtsHighlight.ts`, PDF highlight/reveal path | Optional following control and pending-scroll cancellation. | Omit the optional policy to retain baseline automatic following; keep unrelated highlight fixes. |
| `src/tts/components/AudioControls.tsx` and existing styles | Optional viewport/selection/beginning/return actions and localized live status. Controls portal into the reader shell outside its auto-hiding PDF header; Return is a contextual button inside transport; passage commands use a responsive icon-and-label popover group. Selection uses focus snapshot/cleanup hooks; its optional snapshot callback reports availability. Popover fitting owns a scoped ResizeObserver/viewport listeners; close/unmount removes them. Transient notices remain live but are visually clipped during capture or an open popover. | Omit `playAnywhere` and restore baseline idle play wiring; remove unused passage/return glyphs, feature keys and passage/return/notice styles. Retain general popover fitting if desired. Retain general fullscreen/focus/responsive fixes if desired. |
| `src/i18n/locales/*.json`, `src/tts/diagnostics/TtsDiagnostics.ts` callers | Localized labels/reasons; bounded feature-prefixed diagnostic events. | Remove unused keys/event call sites; keep shared diagnostics infrastructure. |
| This plan, README and TTS documentation links | Design, acceptance receipts and user behavior reference. | Mark withdrawn/remove links so documentation cannot advertise removed behavior. |

Feature-specific imports, event labels and tests use a searchable `playAnywhere`
or `play-anywhere` name. Do not sprinkle behavior behind anonymous booleans across
unrelated modules. Optional capabilities at the composition boundary are the
removal seam; no permanent settings toggle, remote flag service, or new config
file is needed. Removal must not require deleting audiobooks, undoing a database
migration, or changing bookmarks/search/import/export.

## 4. Performance and diagnostics

Build structural indexes lazily when saved audio is usable and the feature is
invoked, or during an existing idle opportunity. Opening a huge book must not
immediately build all chunk DOM ranges. Snapshot the target first so deferred
work cannot change what the user chose. Cache by view/chunk revision and release
node references on invalidation/unmount. Reuse the existing mutation invalidation
approach; avoid duplicate observers when an existing owner can supply a revision.

Normal warm lookup is binary search over ordered HTML spans, or a page/item
lookup for PDF followed by bounded span search. No book-wide DOM walk, geometry
scan, text matching, native preparation, or audio read on every scroll/timeupdate.
Cold structural/legacy reconstruction may scale with document size; perform it
once per revision and keep it interruptible/deferred as needed. A simple first
implementation is acceptable only if representative measurements meet the gate.

Proposed product acceptance budgets (targets, **not measured claims**):

| Measurement | Initial acceptance gate |
| --- | --- |
| Visible action feedback | Within 100 ms of activation on the recorded reference devices. |
| Warm capture + chunk resolution | p95 <= 50 ms over at least 30 recorded actions per tested format/device; no feature-attributable main-thread task > 50 ms. |
| Warm request to audible playback | p95 <= 500 ms for local saved audio with reusable mobile track, at least 30 actions; verify actual audible start separately from `play()` resolution. |
| Cold preparation / legacy recovery | Record phase durations and longest main-thread task; show truthful loading and remain cancellable. Report mobile track stitching separately from warm seeks. No blanket cold-start latency promise. |
| Repeated navigation | Existing desktop bounded WAV window remains bounded; released views leave no retained feature observers, timers, DOM snapshots or Blob URLs. |

Record hardware, OS/WebView, app commit/build, synthetic book size/chunk count,
warm/cold state, sample count, method and failures. A budget miss requires a
documented fix or explicit product decision before claiming acceptance. Do not
add benchmark infrastructure or tune chunk sizes just to meet these targets.

Reuse existing bounded TTS diagnostics. Add `play-anywhere` events for capture,
resolution strategy/result/reason, dispatch, seek/play result, cancellation/stale
rejection and follow transitions. Correlate through a session-local request id;
record format, chunk index/id, counts and phase durations. Avoid book prose,
selection text, titles, document URLs, filesystem paths and per-scroll/per-word
events. Diagnostics remain local and contain no new telemetry. Unit tests should
assert reason/phase outcomes where helpful, not fragile log strings everywhere.

## 5. Step-by-step implementation checklist

Each stage ends with evidence in this document. Keep changes reviewable by
boundary; passing a unit stage does not close packaged listening acceptance.

### Stage 0 — Baseline and contracts

- [x] Recheck branch, working tree, all player/capability callers and relevant
  current tests. Preserve unrelated/private files.
- [x] Confirm canonical chunk filtering, source revision ownership, PDF/OCR
  text-layer mapping, and existing context-menu/selection integration point.
- [x] Finalize the ownership/optional contract design above and implement the pure
  resolver first; concrete viewer registration remains Stage 2.
- [ ] Select generic fixtures/reference devices and capture current saved-audio
  identities/bytes and basic transport behavior as the compatibility baseline.

Acceptance: each integration has one owner and removal action; no format/schema
change is needed; fixture identities are independent of personal imported books;
the untouched baseline can play/pause/resume/skip/stop.

Code audit/contract preparation is complete. The last checkbox and real transport
baseline remain pending: helper tests do not exercise audible playback. Generic
source fixtures are selected below; no personal library/audio files were read.
The baseline had no reader selection/context action API to extend. Stage 5 now
adds an explicit audio-menu action and optional capture capability while retaining
the platform's normal text-selection menu.

### Stage 1 — Source-to-chunk resolution

- [x] Add typed resolved/unavailable outcomes and pure containment/index lookup.
- [x] Cover exact starts/ends, multiple chunks per segment, multi-segment chunks,
  empty chunks, whitespace, invalid/non-finite offsets, unsorted/invalid spans,
  repeated phrases, Unicode surrogate pairs and Arabic diacritics/RTL.
- [x] Test that the existing validated legacy span graft feeds the resolver
  without changing imported ids/text/hashes. Runtime live-DOM recovery wiring is
  part of Stage 2, not a second reconstruction algorithm in the resolver.

Acceptance: known fixture points map to the expected canonical chunk id/index;
ambiguous/unmapped/invalid input cannot become chunk zero; resolver tests run
without DOM/native/React. Prepared chunks already exist while playback is idle
inside `useAudiobookDocument`; exposing them to the reader is Stage 2. Stale
view/session validation belongs to the adapter/coordinator in Stages 2–3.

Implemented contract: `createChunkSourceResolver(chunks, segmentLengths)` takes
the complete source chunk list and normalized UTF-16 segment lengths. It returns
`ready` with `resolve({segmentIndex, offset})` or typed `unavailable`. Construction
snapshots bounds/id/index, filters blank chunks in playback order, and rejects
missing, out-of-bounds, unsorted, overlapping or duplicate-id spans. Normal lookup
uses binary search. It does not verify live text identity, normalize DOM points,
skip whitespace or detect staleness: adapters must supply validated/recovered
spans and normalized points, and rebuild on source/view revision changes.
Partial span coverage intentionally remains unavailable until recovery succeeds.
No production caller imports the resolver yet.

### Stage 2 — Viewer capture and PDF/OCR mapping

- [x] Implement optional HTML capture registration/cleanup through HtmlViewer and
  DocumentViewer. With no callback consumer, no capture observers/index work run.
- [x] Implement the corresponding optional PDF capture registration/cleanup;
  leave the App consumer absent until source validation and coordination exist.
- [x] Map live HTML Text-node points back into normalized narration segments;
  verify against existing forward Range mapping. Capture visible HTML in source
  order beneath chrome, including columns, RTL, enlarged text and nested scrolling.
- [x] Capture first visible PDF text beneath occluding controls in source order;
  verify nested containers, fullscreen-style geometry and native PDF zoom/all
  right-angle rotations, plus RTL OCR and image/text boundaries in Chromium. Physical mobile/WebView acceptance
  is still open; no actual native fullscreen acceptance is claimed.
- [x] Reverse-map PDF text items and merged OCR lines to source word offsets;
  handle missing virtualized layers as not-ready. Verify PDF Range capture with both, fallback-only and
  neither caret API available. Add isolated page/item chunk lookup with
  half-open ranges and earliest canonical ownership of shared glyphs.
- [x] Test HTML formatting/Find-style marks, immediate text mutation, URL
  replacement/unmount, pending scroll/reflow, document hiding, detached root,
  repeated capture and disposal. Test standard and fallback caret API branches
  in Chromium; actual WebKit/device checks remain pending.
- [x] Connect prepared chunks and validate live chunk/span identity. Reuse the
  existing ordered legacy reconstruction when durable spans are unavailable;
  do not feed merely length-valid, stale spans to playback.

Acceptance: rendered-browser fixture tests assert actual geometry/selection
mapping rather than relying on jsdom layout. Supported visible passages resolve
to expected chunks after reflow/Find/zoom; unsupported pages report unavailable.
Absent capability props leave ordinary reading behavior unchanged.

HTML implementation details: `createHtmlTextTargetApi(root, getViewport)` owns a
lazy structural index and browser IntersectionObserver for readable owners.
Only owners currently intersecting the viewport receive text geometry checks;
no book-wide geometry walk runs on scroll or timeupdate. Fresh ancestor clipping
is applied because cached intersection rectangles can move without a new observer
notification. Caret hits from both ends of each visible fragment are compared in
source order, avoiding an assumption that left-to-right screen order equals
narration order. The existing WebKit caret fallback is feature-detected.

`readerTextViewport` intersects the reading surface with the visual viewport and
conservatively excludes header/Find/player rectangles across the reader width.
This intentionally avoids selecting obscured content alongside floating controls.
`normalizeDomTextPoint` replays existing segment whitespace normalization and
advances a whitespace/boundary caret to the next narrated character in that same
segment. It does not modify text, chunking, DOM or stored metadata.

The asynchronous viewport contract refines the original synchronous-text-point
proposal: browser visibility is delivered after layout, so viewport geometry is
captured first and must remain stable through the initial two-frame wait. If
browser visibility has not arrived for every observed owner, wait up to 500 ms
from capture start, then return not-ready. Never mistake an undelivered initial
observation for an empty viewport. No click is
silently redirected to a later scrolled position. New captures supersede pending
ones; hiding, disposing or replacing the view cancels pending work. Unexpected
capture errors reject the Promise. `isCurrent()` drains queued mutations before
checking identity, so even an immediate DOM replacement invalidates old output.

Known limits: one oversized Text node requires its complete line-rectangle list,
and inverse normalization scans its containing segment. Those costs need the
planned performance gate before release; no benchmark claim is made here.
Selection UI and real EPUB/native WebView smoke remain pending. Stage 3 now
dispatches verified targets through the existing player as described below.
PDF/OCR capture is implemented separately as described below.

PDF implementation details: `createPdfTextTargetApi(viewer, container, eventBus,
getViewport)` uses the pinned PDF.js `_getVisiblePages()` result, reorders its
visible-area ranking into page/source order, and visits only those pages. It
intersects reader chrome bounds with the nested PDF scroll container and ancestor
clips. Completed native text layers use `_textHighlighter.textDivs` and original
`textContentItemsStr`, including nested Find nodes. OCR overlays use their original
word orders, page identity and UTF-16 offsets through `pdfOcrSourcePoint`.

PDF native caret hits can miss transformed text or land on overlapping transparent
text layers. A visible-item Range scan finds the first source glyph directly, without
performing duplicate caret hit tests. This supports PDF capture when neither caret
API exists. It does not measure every chunk or request offscreen
pages. A very large single PDF text item can still require a glyph scan: actual
capture latency remains a release gate, including low-end devices.

Capture is synchronous before the returned Promise settles. Rendering, Find,
scale, rotation and page-destruction events invalidate snapshots; hidden documents,
disposal, detached/replaced layers, changed original item text and immediate DOM
text changes are checked before reuse. Missing earlier visible layers stop capture
as not-ready instead of selecting later-page audio. Unknown/stale text mappings
return unmapped. Blank completed pages return no-visible-text. Disposal removes
all feature event listeners. No extra page rendering, narration loading, audio
commands, session queue, dependency or persistent format is introduced.

`createPdfChunkSourceResolver` indexes copied runtime spans by page/item. It preserves
the canonical nonempty playback sequence, checks integer/ID/span validity, applies
half-open containment and chooses the earliest canonical chunk for a shared source
glyph. It visits only that item's candidate spans on lookup. Like the HTML resolver,
it requires validated source metadata from its prepared-target caller: structural checks
alone do not prove live text identity. Both resolvers serve source-only preparation; the separate Stage 3 coordinator dispatches their verified output through the player.

Stage 2c — verified preparation (implemented):

- [x] Expose the prepared canonical chunk sequence independently of active playback,
  retaining runtime PDF source spans and imported authoritative IDs/text/order.
- [x] Validate chunk/span text against live narration source and reuse the existing
  ordered exact-first/Arabic-aware legacy reconstruction without a second matcher.
- [x] Cache validation by actual view/chunk revision; do not reparse/hash the book
  on every action or accept merely length-valid stale spans.
- [x] Test repeated text, mismatched/stale source, old imported bundles, unavailable
  mappings and source replacement during awaited preparation. An invalid capture
  or identity mismatch must produce no audio side effect.

Acceptance for Stage 2c: a captured HTML/PDF/OCR point resolves through verified
prepared chunks to the expected canonical chunk ID/index before any player call.
A legacy bundle preserves its saved sequence and audio; missing/ambiguous identity
returns a typed unavailable result. Existing playback and highlight paths still pass.

Implementation details for Stage 2c:

- `useAudiobookDocument` stamps prepared state with document URL, source HTML and
  model. The first committed render of a different document/model cannot expose
  an old array while its preparation effect catches up. It keeps imported manifest
  chunks authoritative; generated display fallback after a metadata error is not
  eligible for target preparation. `getCanonicalChunks()` retries the manifest.
  Existing reader/highlight fallback remains available without changing saved data.
- `usePreparedTextTarget` owns one source preparation object for the committed
  document/content/model/chunk revision. Manager exports its action and canonical
  prepared chunks independently of `ttsState`. App still does not consume them.
- `createTextTargetPreparation` accepts canonical chunk, PDF narration and PDF page
  loaders. It checks document URL and captured view identity before work, after
  each await and before returning. Returned resolved output includes the original
  full chunk array (including original IDs/text/hashes/order); recovered spans are
  confined to a disposable index. A model/view/chunk replacement invalidates old
  returned `isCurrent()` callbacks as well as pending results. Unexpected loader
  errors reject; a failed manifest/page read can be retried.
- HTML validation compares the complete live normalized segment snapshot to the
  selected source HTML once per view/chunk pair. It then checks source bounds,
  uniqueness/order/overlap, each chunk's actual span text and full source coverage.
  Only whitespace, existing Arabic display-mark compatibility and source-proven
  punctuation discarded by sentence splitting can occupy gaps.
  This rejects equal-length edits and extra/missing repeated occurrences instead
  of silently assigning them to the first available match. Only imported HTML may
  use shared ordered legacy recovery; no import re-chunking is used for targeting.
- The highlighter and feature call the same `recoverOrderedSourceSpans` helper,
  preserving the forward cursor and exact-first, Arabic-aware match policy.
  Tolerant character maps now retain every UTF-16 unit around supplementary
  characters/NFKC expansions. Matching tolerates spaces inserted by existing
  sentence/clause splitting after terminators, clause separators and closers,
  including decimals and initialisms; ordinary word spacing is still significant.
  Source text and audio/cache identities are untouched. The full tolerant offset
  map is built only if matching needs it.
- PDF validation checks normalized narration and source-run coverage/bounds, then
  requires each runtime PDF span to equal the production `pdfSourceSpansForChunk`
  projection of that chunk. The captured page's original item/word text must match
  its persisted block and page/order uniquely; native `hasEOL`'s appended newline
  is the only allowed suffix difference (OCR newlines remain exact). Source offset
  and candidate-span bounds are checked against the original block. Ligatures and
  nonlinear source runs retain earliest canonical chunk ownership.
- Warm HTML requests reuse the snapshot-array/chunk-array validation result,
  including failures. PDF narration/projection validation is cached by chunk
  revision. Concurrent canonical preparation shares one promise; failures clear
  it for retry. Only the last requested PDF page is cached, bounding retained page
  data. No per-action book hashing, re-chunking, all-page reading/rendering or
  geometry scan is introduced. Initial validation still visits source/chunks;
  large-book CPU/memory and device latency must pass the existing release gate.

These guards verify source-to-canonical-chunk mapping. They do not establish that
selected audio is complete or that playback options/action intent are unchanged.
Stage 3 now binds the target to saved-audio options and its canonical sequence,
invalidates stop/newer actions synchronously, rechecks `isCurrent()` after audio
preparation, and dispatches through the existing player serialization. Controls and
follow behavior are implemented in Stage 4 below; selection follows in Stage 5.

### Stage 3 — Initial start and active navigation

- [x] Add backward-compatible starting index to the existing player entry point.
- [x] Desktop loads/plays the target chunk first and keeps current prefetch bounds.
- [x] Mobile awaits source/rate/target seek before play; explicit active
  seek-and-play works while paused and after foreground synchronization.
- [x] Coordinate capture/resolve/dispatch with document/audio identity and newest
  request guards. Preserve full canonical sequence and native signature.
- [x] Test rapid A->B->C targets, stop/close/model change during each await, late
  native replies, missing/corrupt chunk, preparation and seek/play rejection.

Acceptance: first audible content is the requested chunk's beginning; no chunk
zero blip. Paused Play from here actually plays, Resume retains exact position,
newest intent wins, and failed resolution leaves current playback intact.
Existing callers without a start index preserve their behavior.

- [ ] Complete physical desktop/Android/iOS listening acceptance for actual
  first audible content, native seek accuracy, foreground/lock-screen behavior
  and latency. Automated command/state tests do not close this gate.

Implementation details for Stage 3:

- `usePlayAnywherePlayback` captures before its first await and checks saved-audio
  completion/document URL, newest intent and verified target after capture and
  source preparation. It passes full canonical chunks/options/index and an
  `isCurrent` predicate into `playFromChunk`. Failed capture/resolution invokes
  no transport mutation. Errors before dispatch are returned to the caller;
  transport errors remain in `ttsState`. A dispatched result is not audible-start
  confirmation; Stage 4 consumes the result and existing loading/error state.
- `useAudiobookManager` exposes `playFromHere`; Stage 4 wires App/viewer controls.
  Existing Read routes through guarded `playFromBeginning`; Stop, close, preview,
  model/saved-audiobook selection and transport controls synchronously invalidate
  pending target actions. Voice, preprocessing, quality/thread changes cancel
  pending requests before updating settings. Committed option/source changes
  and unmount dispose the scope. No second audio queue or feature setting exists.
- `useTtsPlayer.speak` filters the full sequence as before, rejects invalid target
  indices before pausing/replacing audio, and defaults to zero. Desktop begins
  through its existing loader at the requested index with local time zero, then
  continues normally; its two-chunk speculative prefetch and bounded Blob cache
  remain unchanged. Superseded load failures, old ended events and capability
  preloads cannot overwrite a newer job. Pause cancels queued navigation;
  Resume retains the active audio's local time.
- Native initial start prepares the existing full seekable track, validates its
  complete chunk ID/index/timing map, initializes the existing session, sets
  source and rate, awaits seek to the selected chunk's actual `startSec`, then
  plays. No zero-start state is published before that seek, and no opening audio
  is played as a route to the target. Pending source guards are checked after
  every await and at actual queued mutation execution. Guard release at the first
  accepted play command lets later browsing leave ongoing audio intact.
- Native timing validation uses the existing **0.05-second manifest tolerance**
  from `native_tts/engine/manifest.rs`; stitched track totals pass through f32.
  This is metadata compatibility, not an estimated seek offset. The seek always
  uses the actual stored `startSec`. Finite positive durations, canonical IDs/
  indices, nonnegative ordered boundaries and full track length remain required.
- Active `playFromChunk` uses source-array identity as the common fast path,
  otherwise compares canonical nonempty IDs/text/hashes once for the changed
  array, plus audio options. Matching sessions use existing navigation without
  preparing or setting the native source again. A changed identity/options starts
  a new job. Mobile explicit seek-and-play resumes a paused session after seek;
  ordinary mobile jump retains paused behavior. Relative skips accumulate through
  shared foreground synchronization instead of dropping rapid taps.
- `nativeMobileAudio` retains its single lifecycle queue. Optional predicates for
  source/rate/seek/play/pause run after plugin loading immediately before the
  mutation, so stale commands waiting behind another operation abort without
  reaching the plugin. An aborted operation does not poison later queue work.
  Hook job/navigation fences also reject late replies; an old seek worker cannot
  clear a replacement job's state. Teardown invalidates generations before
  releasing audio/cache/session resources, including initialization in flight.

### Stage 4 — Controls and follow/browse behavior

- [x] Wire stopped Play from here, explicit Play from beginning, and browse-mode
  Play from here / Return to audio through existing controls.
- [x] Separate following from playing/paused state; gate HTML delayed scroll,
  PDF page changes and every automatic audio-highlight reveal.
- [x] Handle wheel/keyboard/pointer scroll intent, scrollbar dragging, selection
  dragging and explicit Find/bookmark/link/top/PDF page/outline jumps. Ignore bare
  programmatic scroll events; cancel/guard pending reveals; dispose listeners on idle/teardown.
- [x] Respect reduced motion in audio reveals; retain ordinary Resume/transport.
- [x] Add all eight locale keys and accessible loading/unavailable messages,
  explicit fallback, menu focus handling and responsive/fullscreen dock layout.
- [ ] Complete real touch/momentum, fullscreen WebView, RTL/enlarged-text,
  keyboard/screen-reader and physical listening acceptance. Synthetic Chromium
  layout/input and mocked transport tests do not close these platform gates.

Acceptance: scrolling ahead never snaps back at the next chunk; highlighting
continues. Return to audio reveals without seeking or resuming paused playback.
Successful Play from here restores following, with later user browsing taking
precedence. Controls remain usable in mobile/fullscreen/RTL/enlarged-text layouts.

Implementation details:

- `App` binds one `usePlayAnywhereReader` instance to the selected document and
  passes its optional policy/capture registration/input callback to DocumentViewer
  and action/state props to AudioControls. It wraps transport actions with UI
  cancellation; the manager retains authoritative source/transport cancellation.
  No setting, persistent listening history or second audio controller was added.
- The reader hook keeps following independent of audio play/pause, resets the
  session on document replacement/Stop, and records browse intent synchronously.
  A pending Play from here holds its browse generation. Following resumes only
  after its guarded `onStarted` callback, and only if no newer browse input won.
  Source/action cancellation also invalidates that callback. Transport Promise
  resolution is an acknowledgement, not proof of audible playback/seek accuracy.
- `AudioFollowPolicy.allowsReveal()` is checked when an HTML delayed timer or
  PDF rendered-layer callback actually executes. While browsing, HTML/PDF still
  update their highlights, including newly rendered OCR layers, but PDF never
  assigns the audio page. Return changes policy revision, allowing the same
  paused chunk to be revealed without any player call. Reduced motion uses an
  immediate reveal; existing behavior remains the default without a policy.
- `bindReaderBrowse` installs scoped document input listeners. Wheel excluding
  pinch-zoom, navigation keys outside form controls, a pointer move beyond five
  CSS pixels and scrollbar presses suspend following. Pointer tracking covers
  touch pan/selection dragging; following remains suspended through subsequent
  momentum scrolls. Bare scroll events never infer intent or reset follow. Find,
  bookmarks, internal/annotation links, top, PDF page buttons and outline jumps
  report explicit browsing at their owning boundary. Input listeners exist only
  during an audio session or pending explicit action and detach on Stop/idle or
  view teardown. No book geometry is walked on these input events. Actual mobile momentum and platform scrollbar behavior
  still require device acceptance.
- The idle primary action uses viewport capture. Resume keeps its existing time;
  Return has a visible label inside transport; viewport, selection and beginning
  commands live in the existing audio popover, including while stopped. Failed capture/resolve never invokes a
  fallback automatically. Pending feedback remains until play acknowledgement,
  so the live region announces explicit action work rather than every chunk or
  time update. During HTML's asynchronous capture, status remains available to
  assistive technology but is visually clipped and the idle button uses its fixed
  size spinner. Growing the dock then would invalidate the viewport geometry
  snapshot. Once capture settles, the notice may expand normally. Actual player
  failures remain visible, and retries remain usable.
- The floating dock is portaled into the current reader shell, outside the
  transform/opacity of PDF's auto-hiding header and inside native fullscreen.
  Return reflows within transport. Transient notices sit above transport when
  the popover is closed; opening it visually clips the notice while retaining its
  live status. Viewport geometry excludes the full dock. Menu actions receive focus; Escape
  and selection restore the trigger. Disappearing browse/start actions transfer
  focus to surviving transport controls rather than the document text.
- Diagnostics use the existing local bounded logger for `[play-anywhere]` action
  intent/result and follow changes. Only request IDs/action/status/reason/booleans
  are recorded; no prose, URLs, titles or paths. No per-scroll logging occurs.

### Stage 5 — Selection entry point

- [x] Add the explicit **Play from selection** audio-menu action without replacing
  normal selection, Copy, link activation or platform context/accessibility gestures.
- [x] Capture before focus loss; validate both endpoints/current document and map
  the Range's document-order start through the same resolver/coordinator.
- [x] Test keyboard Tab/menu activation, pointer activation with touch metadata,
  backward/multi-node and element-boundary selections, whitespace, skipped
  references, hidden text, cross-reader ranges and invalidation while preparing.
- [x] Test real PDF.js Find-split text and OCR line-to-word source offsets; retain
  viewport targeting independent of incidental native selection.
- [ ] Verify physical touch long-press/selection handles, platform Copy menus,
  screen readers and actual chunk-start audio on the device/format matrix.
  Browser Selection/PointerEvent fixtures do not establish these platform gates.

Acceptance: selecting anywhere within a chunk starts that chunk from zero;
viewport actions remain independent of incidental selection. A keyboard user
can invoke the action and recover focus; unavailable selections explain why.
The explicit audio-menu action is available by ordinary keyboard and touch button
activation. OS context menus are preserved; no OS-menu insertion is claimed.

Implementation details:

- `ViewerTextTargetApi.captureSelection?()` is synchronous so the source point can
  be frozen before controls receive focus. Adapters return the existing captured
  HTML/PDF result or typed unavailable reason. No new manager/player entry point,
  native command, audio format or timing calculation was introduced.
- `readerSelection` requires one noncollapsed range with **both endpoints inside
  the current reader**; Range start/end supply source order even for backward
  selections. `selectedTextStart` walks forward from the start boundary, handles
  element/end-of-Text boundaries and whitespace, and stops at the selected end.
  It never calls `Selection.toString()` over an entire selected book. The first
  substantive character must map to narrated text; an excluded reference or
  hidden start returns unmapped instead of jumping to unrelated audio.
- HTML reuses its lazy structural index and normalization; a Text-to-segment map
  avoids a whole-book search on subsequent selection actions. It needs no caret
  geometry or IntersectionObserver availability for selection capture. The source
  arrays/index remain owned by the existing view revision and are not serialized.
- PDF selects only an already-rendered page identified by its actual selected
  node. It uses the pinned item map through nested Find marks, or OCR's transient
  line-to-original-word map. Viewport and selection capture share one PDF point
  validation/lifetime guard. No page rendering or full-book PDF scan is requested.
- `usePlayAnywhereReader` retains at most one menu snapshot. Menu pointerdown and
  keyboard Tab snapshot before default focus changes. Opening by keyboard may
  reuse that snapshot when native selection collapses, including when Tab passes
  through intermediate reader controls.
  The snapshot is consumed by the explicit action, or released on menu dismissal,
  transport cancellation, capability replacement or reader teardown.
- New non-audio pointer/non-Tab key input invalidates a pending selected target via
  a separate epoch. Different noncollapsed live ranges, source mutations, hidden/
  disposed views, PDF layer/scale/rotation changes and cross-reader boundaries
  also fail guards. Collapsed native selection alone does not discard the frozen
  point; input ownership and menu lifetime determine whether it may be retained.
  Closing the menu after activating its action releases the UI snapshot without
  canceling that already chosen target. Async preparation still rechecks its
  guard before touching the existing player.
- Selection is passed to the existing `playFromHere` coordinator as a captured
  source result; viewport actions still call `captureViewport()` directly.
  Completed saved-audio gating, full canonical sequence, chunk-zero offset,
  newest-action guards, paused seek-and-play and follow-on-start remain shared.
  **Play from selection** means continue the audiobook from that selected passage,
  not speak only the selection. A missing captured selection disables the menu
  command and shows the existing localized guidance. A changed/invalid target
  still fails its action-time guards; beginning/viewport fallback always requires an explicit action.
- Both new strings exist in all eight locales. The ordinary menu button has a
  visible accessible label, receives existing menu focus and restores the trigger
  on activation/Escape. There is no new global shortcut, selection-clearing call,
  replacement text menu, gesture suppression or diagnostic logging of selected prose.

### Stage 6 — Regression, accessibility and performance gates

- [x] Add focused resolver, adapter, coordinator and player lifecycle regressions
  for new behavior, including baseline behavior with optional capabilities absent.
- [x] Run relevant focused tests, then `npm test`, `npm run build:typecheck`,
  `npm run check:i18n`, `npm run lint`, and `npm run build` before integration.
  Run native tests/checks only if native code changes; retain existing CI matrix.
- [ ] Complete the device/format matrix below and performance budgets above.
- [ ] Verify unchanged saved source/audio/chunk identity fixtures and existing
  bundle import/export, transfer, Find, bookmarks, model settings and lock-screen
  controls. No private corpus excerpts enter tracked fixtures or diagnostics.
- [x] Review dependency direction, cleanup, error boundaries and removal scope;
  record any added/changed integration surfaces here.

Acceptance: no failing required check, no unexplained regression, no unmeasured
performance claim. Relevant WCAG checks pass with keyboard and screen reader;
device listening confirms target content and ordering. Build/CI success alone
does not establish audible, accessibility or performance acceptance.

### Stage 7 — Removal rehearsal and release decision

- [x] In a disposable patch/worktree, disconnect the optional feature coordinator,
  restore the original idle-play action and omit follow-policy/start-index inputs.
  Do not reset/delete user work to rehearse removal.
- [ ] Verify baseline playback/reading still works without any feature-owned
  runtime module; check compile, focused baseline tests and a playback smoke.
- [ ] Confirm removal needs no user-data deletion, schema downgrade, cache
  regeneration or changes to import/search/bookmarks/transfer.
- [x] Restore the intended feature patch and rerun checks affected by restoration.
  The 2026-10-22 rehearsal used disposable copies, so no live restoration was
  needed; the intact feature checkout was checked independently.
- [ ] Update this checklist, current TTS behavior documentation and README with
  actual results/limits. Record exact accepted commit, devices and owner listening
  result; leave untested configurations explicitly pending.

Acceptance: removal steps are executable and bounded to the listed seams; the
owner can open an existing book, scroll into its middle and hear the correct
saved chunk beginning. Do not merge/release while required gates remain open.

## 6. Acceptance matrix and evidence record

| Configuration/case | Required evidence | Status |
| --- | --- | --- |
| HTML + converted EPUB/TXT/Markdown | Top visible point; mid-paragraph; multiple chunks; images; Find; font-size/reflow; first/last chunk. | Pending |
| Arabic/RTL + mixed direction | Diacritics/normalization; inline formatting; links/references; source-order targeting. | Pending |
| Embedded-text PDF | Columns, zoom/rotation, page boundary, virtualized text layers, Find-split DOM. | Pending |
| Recognized-text/hybrid PDF | OCR point -> source run -> chunk; unmapped image-only page; interrupted readiness. | Pending |
| New and legacy imported bundles | Valid durable spans; ordered reconstruction; repeated text/ambiguity; unchanged canonical audio identity. | Pending |
| Linux packaged AppImage | Real audible start, no opening blip, browse/follow, repeated seeks, cold/warm preparation. | Pending |
| Windows + macOS native WebViews | Viewport/selection capture, transport, cleanup and unavailable fallback. | Pending |
| Android + iOS physical devices | Touch/selection, paused seek-and-play, background/foreground, lock-screen controls and interruption. | Pending |
| Keyboard/screen reader/reduced motion | Accessible names/status, focus order/return, menus, target sizes, no forced focus/scroll. | Pending |
| Repeated lifecycle + removal rehearsal | Stop/close/switch/cancel races; released resources; baseline with feature disconnected. | Structural removal passes; packaged/device smoke pending |

For every completed gate append: commit/build; environment/device; fixture and
size; exact checks/sample count; result; evidence location; known limitation.
Do not check a platform row solely because another platform's unit tests passed.

Planning evidence: code inspected on the stated baseline; prior investigation
ran 12 existing helper tests in four files (playback state, browser audio cache,
word timing, PDF mapping). That receipt predates this branch's implementation
and does not validate the new feature. MemSearch recall/expansion confirmed the
need to distinguish compilation from real listening/device acceptance; existing
compatibility requirements are also documented in [native TTS](kokoro-tts.md).
The initial planning documents were committed as `f138e02` on this branch before
source work. The resolver receipt below is implementation evidence only; no
device/performance acceptance is recorded yet.

### 2026-10-22 — baseline audit and isolated resolver foundation

- Baseline: planning commit `f138e02`, based on main `8ebc9d0f`; Linux local
  Node/Vitest environment, no running player or physical-device test.
- Callers inspected: the manager is the sole `useTtsPlayer` consumer and its
  `speak` caller; controls consume its existing read/resume/jump props. Both
  player and native source signature filter nonempty chunks in original order.
  Existing viewer APIs cover Find/bookmarks, not selection actions.
- New code is confined to `src/tts/playAnywhere/sourceChunkResolver.ts` and its
  test. It imports existing chunk types only, runs without DOM/React/native
  globals, and is not wired into reading/playback. Removing these two files
  completely removes this stage's runtime code.
- Fixtures are generic in-memory spans plus a 40-paragraph synthetic book built
  with the production chunker (English repeated sentences and marked Arabic).
  Binary lookup is compared with independent linear containment at every source
  character; separate cases cover UTF-16 emoji and multi-segment spans.
- Original imported chunk ids/text/hashes and chunker output are asserted
  unchanged; legacy graft id/text mismatch still rejects. No saved WAV/source
  bytes, manifests, cache/version, preferences, dependencies or native code change.
- Validation: the original focused baseline passed 17 tests in seven files.
  Final focused run passed 60 tests in eight files, including 43 resolver tests;
  these counts overlap rather than sum. `npm run build:typecheck`, focused ESLint,
  local Markdown link checks and `git diff --check` passed. New-file whitespace
  was checked separately because the new module/test remain untracked until
  committed. Full frontend/native suites, packaging and runtime smoke were not
  rerun for this isolated module.
- This stage does not establish rendered viewport targeting, audible transport,
  performance budgets, OCR mapping, race handling or accessibility acceptance.
  Stage 0's physical baseline remains open alongside those later gates.
- Next implementation: Stage 2, starting with HTML viewport/selection point
  conversion and optional viewer capability, then PDF item/OCR inverse mapping.
  Keep player dispatch and visible controls deferred to Stages 3–5.

### 2026-10-22 — Stage 2a optional HTML viewport capability

- Previous resolver work committed first as `7790bf2`; the HTML delivery was
  subsequently committed as `5e04f81` before Stage 2b.
- Added only the HTML adapter, shared inverse point helper, viewer types and
  optional registration/bounds wiring. App does not register a consumer, and
  existing reader/player behavior remains the baseline. No audio/cache/native
  code, storage, dependency, localization, build configuration or CI change.
- Added DOM round-trip/whitespace/reference tests and React lifecycle tests
  proving no optional-capability setup without a consumer and disposal even when
  the URL changes while body content stays identical. A viewport-provider failure
  is verified to reject the capture Promise rather than throw synchronously.
- Real Chromium tests bundle the actual helper modules with the installed Vite
  API and use disposable local fixtures/profile. A DevTools pipe awaits real
  asynchronous layout (dump-dom virtual time was insufficient for reliable RAF
  completion). Geometry is compared with an independent per-glyph Range oracle;
  captured source points also resolve through the production chunker and resolver.
- Ten browser cases cover narrow/enlarged RTL, two columns, long paragraphs,
  caret fallback, hidden/reference text, nested scroll/reflow, mutation, scroll,
  replacement/disposal, hidden document/detached reader, missing caret APIs and
  an image-only viewport. The fallback branch running in Chromium does not
  establish actual WebKit support. Tests follow the existing local Chrome skip /
  CI-required browser convention and add no package dependency.
- Environment: Linux, Node `v24.21.0`, Chromium `155.0.8059.84` (Arch Linux).
  Fixtures/profile remain disposable; no installed application or user library
  was opened or modified.
- Validation: focused capture/resolver tests passed 63 in four files. Final full
  frontend suite passed 436 with one existing skip across 63 files, including
  the additional Promise-error regression; focused/full counts overlap.
  TypeScript, focused
  ESLint, documentation link/whitespace checks and diff checks passed. Chromium
  needed its usual sandbox exception for local process sockets. No native suite,
  packaging, listening, device/AT or performance acceptance is claimed.
- Next: finish Stage 2 with PDF.js item capture and transient OCR line-to-source
  mappings, then verify live prepared-chunk/legacy identity integration. Audio
  start/seek and visible controls remain Stages 3–5. Stage 0 physical baseline
  and the full platform acceptance matrix are still open.

### 2026-10-22 — Stage 2b optional PDF/OCR viewport capability

- Committed the reviewed HTML stage first as `5e04f81` on `feature/play-anywhere`.
  The PDF/OCR delivery was subsequently committed as `771c940` before Stage 2c.
- Added the optional PDF factory/registration, original PDF item/word capture,
  transient OCR line-to-word mappings and isolated PDF chunk index. Shared only
  DOM geometry and the existing Chromium fixture runner with the HTML tests.
  Hardened initial HTML visibility against delayed browser delivery with a bounded
  wait and not-ready timeout; fixtures await font readiness and PDF initialization
  before asserting capture/registration, rather than depending on import timing.
- Real PDF.js 6.2.108 in Chromium verifies Find, zoom, 90/180/270-degree rotations,
  navigation to page two and fullscreen-style geometry against an independent
  source-order glyph oracle. PDF Range capture runs with both caret APIs available, only the
  WebKit fallback available, and neither available. The HTML tests still exercise
  both caret implementation branches. OCR fixtures verify partial-word capture, original
  newline/emoji offsets, RTL source order, nested marks, production chunker/span
  projection -> chunk lookup, and immediate text changes. Missing/unmapped/blank,
  hidden, detached and disposed cases never choose chunk zero as a fallback.
- React lifecycle coverage verifies optional opt-in, URL replacement, opt-out and
  unmount. A 10,000-page stub permits capture to request only the supplied visible
  pages; this is a bounded-work assertion, not a measured performance benchmark.
- Validation: full frontend suite passed **450 tests with one existing skip**
  across 66 files, including 19 real Chromium HTML/PDF/OCR cases. TypeScript,
  focused ESLint, documentation links and whitespace checks passed. Chromium
  required its local process-socket sandbox exception. No packaging/native suite,
  listening/device/AT or measured latency acceptance is claimed.
- Next: Stage 2c prepared-chunk/live source validation and ordered legacy recovery.
  Playback/control/follow/selection remain Stages 3–5; physical baseline,
  native WebView/device/AT/audio and capture latency acceptance are still open.

### 2026-10-22 — Stage 2c verified canonical target preparation

- Committed Stage 2b first as `771c940` on `feature/play-anywhere`; this source
  validation delivery was subsequently committed as `ad5f055` before Stage 3.
- Added the manager's independent canonical chunk exposure and source-only
  preparation action. Player entry points, native commands, controls and following
  are unchanged; App still registers no capture consumer.
- Shared existing legacy recovery with highlighting, fixed tolerant UTF-16 offset
  maps and sentence-inserted spacing, and made tolerant indexing lazy. Durable
  spans are checked against actual source text/order/coverage, with imported
  recovery preserving original manifest IDs/text/hashes/sequence and audio.
- Added owned preparation caches, view/source/model/chunk invalidation, manifest
  retry without generated import fallback, and persisted PDF item/word checks,
  including native EOL, nonlinear projection and bounds. Renderer snapshots carry
  frozen normalized HTML texts for validation without repeating book parsing.
- Chromium fixtures now pass real HTML/native PDF/OCR captures through the new
  verification boundary to canonical chunks. Other tests cover repeated/extra/
  missing phrases, same-length source edits, Arabic marks, emoji/NFKC, decimals,
  ligatures, invalid runs/projections, cache reuse, failed reads/retries, replacement
  during awaited source/page reads, StrictMode, opt-out and unmount.
- Validation: full frontend suite passed **469 tests with one existing skip**
  across 70 files, including 19 real Chromium cases. TypeScript, focused ESLint,
  local documentation links, fences and whitespace checks passed. Chromium used
  the usual local process-socket sandbox exception. No native suite, packaging,
  listening/device/AT or measured latency acceptance is claimed.
- Next: Stage 3 player integration, including optional initial index, desktop target
  load, real native track start offsets, seek-and-play, and newest-action/stop guards.
  Physical baseline, native WebView/audio/device/AT and measured latency gates remain
  open. No availability, listening quality or performance claim is made here.

### 2026-10-22 — Stage 3 guarded player integration

- Committed Stage 2c first as `ad5f055` on `feature/play-anywhere`; this transport
  integration was subsequently committed as `e16020c` before Stage 4.
- Added optional initial index and explicit active seek-and-play through the
  existing player. Kept canonical full sequences/signatures, imported chunk
  identity, desktop prefetch/cache bounds and ordinary Resume behavior.
- Added the feature-owned newest-intent coordinator and manager entry point,
  synchronous transport/settings cancellation and source/job/navigation guards.
  Native mutations check guards inside the existing lifecycle queue; timing
  validation matches native manifest rounding tolerance. No native command,
  persistent format, dependency, new control or capture consumer was added.
- New tests use mocked native/audio transport plus the actual lifecycle queue
  around a mocked plugin. They verify command order and guards, not acoustic or
  physical-device behavior. Coverage includes default/invalid indices, canonical
  session reuse/replacement, paused seek-and-play versus Resume, rapid absolute/
  relative navigation, stop at each startup phase, foreground sync, stale views,
  old chunk/seek/play/capability replies, missing/corrupt chunks, errors and
  duration-rounding compatibility. Existing Chromium capture/validation coverage
  remains in the full suite.
- Validation: full frontend suite **510 passed, 1 existing skip across 73 files**;
  TypeScript build check, focused ESLint, Markdown fence/local-link checks and
  `git diff --check` passed. No Rust/native build or physical listening run was
  needed to validate this frontend patch; those acceptance gates remain open.
- Next: Stage 4 controls and follow/browse behavior, all eight locale strings,
  loading/unavailable messages and accessible viewport/beginning/return actions.
  Actual first-audible-content, native seek accuracy and device/lock-screen/AT/
  performance acceptance remain open before availability/release.

### 2026-10-22 — Stage 4 viewport controls and independent following

- Committed Stage 3 first as `e16020c` on `feature/play-anywhere`. Stage 4 was
  subsequently committed as `84faec4` before the selection stage.
- Wired viewport play, explicit beginning, browse actions and capture/follow
  policy through existing composition. Kept save/setup, pause/resume/skip/stop,
  canonical chunks, audio formats and native playback architecture intact.
- Added session-local reader intent/feedback and guarded playback-start
  acknowledgement; no source/DOM references persist. Added only optional seams
  to controls/viewers/player. Removal instructions above identify each owner.
- Tests cover synchronous browse intent versus programmatic scroll, delayed HTML
  reveal cancellation with ongoing highlights, paused same-chunk Return, PDF
  page/reveal gating, stale document/action callbacks, successful/late transport
  acknowledgement, explicit fallback without automatic dispatch and menu/focus
  behavior. Real Chromium renders actual controls/CSS at 320px, Arabic 200% text
  and French 200% text with the PDF fullscreen header hidden; it checks visible
  hit targets and browse button size. Another Chromium case mounts the real
  reader hook/controls/HTML capture together and verifies that initial loading
  feedback does not stale a mid-book viewport snapshot. These are synthetic
  fixtures, not native WebView, complete reader workflow or assistive-technology
  acceptance.
- Validation: full frontend **524 passed, 1 existing skip across 78 files**;
  production build (locale parity, TypeScript, Vite, Pagefind), full ESLint, local
  Markdown links/fences and `git diff --check` passed. Existing Vite mixed-import/
  large-chunk warnings remain. No native source/build change was required.
- MemSearch recall ran against this project's collection. It reinforced the
  existing distinction between automated checks and deferred device/listening
  acceptance; current source/checklist determined this stage's integration.
- Next: Stage 5 explicit selection entry point, snapshotting the document-order
  selection start before menu focus changes and using the same verified resolver.
  Stages 6/7 still require the device/format matrix, measured budgets, accessibility
  acceptance and removal rehearsal before a release decision.

### 2026-10-22 — Stage 5 explicit selection action

- Committed Stage 4 first as `84faec4` on `feature/play-anywhere`. The selection
  stage was subsequently committed as `5c7802b` before Stage 6 packaging.
- Added optional synchronous HTML/PDF/OCR selection capture and an audio-menu
  entry using the existing coordinator/resolver/player. Added only shared
  selection validation/input helpers and the existing reader hook's bounded
  snapshot lifecycle. No dependency, native API, persistent format or setting changed.
- Automated coverage includes real Chromium keyboard/pointer menu activation
  with native selection collapsed after focus, PDF.js Find and OCR selection
  offset/verified-chunk resolution, backward/multi-node/boundary/whitespace cases,
  excluded/hidden/foreign-reader rejection, menu focus/notice, pending preparation
  invalidation and unchanged viewport capture with an incidental selection.
- Validation: full frontend **537 passed, 1 existing skip across 79 files**;
  production build (locale parity, TypeScript, Vite, Pagefind), full ESLint, local
  Markdown links/fences and whitespace checks passed. Existing Vite mixed-import/
  large-chunk warnings remain. No native source or dependency changed.
- At the user's request, explicitly read/wrote/read back the project MemSearch
  journal under a file lock. This confirms filesystem access only; automatic
  capture/index/retrieval health was not inferred from it. Private memory files
  remain outside the commit.
- Next: Stage 6 regression/accessibility/performance acceptance, **Linux AppImage
  first**, as selected by the user during this run. Record actual app build,
  fixture, hardware, first audible chunk and cold/warm measurements; complete
  keyboard/screen-reader/reduced-motion checks and retain other platform gates.
  Stage 7 removal rehearsal and release decision remain separate.

### 2026-10-22 — Stage 6 Linux passage-change regression

- Owner testing of the `5c7802b` AppImage found “The passage changed. Try again.”
  when invoking Play from here, including with text selected. Linux acceptance
  remains open; manual testing belongs to the owner and no app automation is used
  for this fix.
- Reproduced the rejection in an isolated Chromium fixture using the actual
  `HtmlViewer`, reader coordinator and controls. The previous fixture used a
  static article and missed the viewer's React updates. Idle, playing and paused
  capture all failed before the fix; a separate viewer regression confirmed
  unchanged text nodes were replaced on an audio-follow prop update.
- Root cause: the viewer supplied a fresh `dangerouslySetInnerHTML` object on
  every render. React rewrote identical book content when audio UI state changed,
  invalidating DOM ranges and the adapter's source revision. Memoize that object
  by body HTML so unrelated renders preserve source nodes, selection, Find and
  highlighting ranges. Actual content replacement and viewer teardown still
  invalidate targets. No stale-target guard is relaxed.
- Integration surface: `src/viewers/HtmlViewer.tsx` only; the tests cover viewer
  node/selection identity, actual source replacement, idle/active/paused capture
  and pointer/keyboard selection-menu activation. Source capture, resolver,
  player, native APIs, saved chunks/audio, dependencies and formats are unchanged.
  The viewer correction also stands independently if play-anywhere is removed.
- Checks: **540 passed, 1 existing skip across 79 frontend files**; full ESLint,
  TypeScript, locale parity, production build (Vite/Pagefind), native AppImage
  packaging, media-bundle verification and whitespace checks passed. Existing
  Vite mixed-import/large-chunk warnings remain. No audible, WebKit, screen-reader
  or latency acceptance follows from Chromium success.
- Rebuilt local artifact:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-passage-fix.AppImage`;
  SHA-256 `582b95d5758cd3c7093707020f446337fa118c107a57f1d3089bcecd94e6ef1b`.
  This is base `5c7802b` plus the uncommitted viewer fix; the neighboring
  `passage-fix-build.json` records the exact code/test patch and runtime-source
  hashes. The earlier named `5c7802b` AppImage is preserved and still has the bug.
- Retest: close the previous AppImage and launch the newly named fixed artifact.
  In the same saved book, scroll away from playback and invoke Play from here
  while playing, paused and stopped. Expect the containing saved chunk's beginning
  without the stale notice. Repeat with an incidental selection: Play from here
  still targets the viewport. For selection targeting, use the audio menu's
  explicit Play from selection; test both forward and backward selection.
  Confirm changing/closing the source or stopping during a request cannot start
  old audio. Leave other device/format, accessibility and timing gates pending.

### 2026-10-22 — Stage 6 imported HTML clause-spacing regression

- Owner reported the unmapped-passage notice from both the top Play from here
  icon and the browse action after Play from beginning. Owner confirmed an
  imported `.papercut-audiobook` bundle with HTML source. Manual app control and
  listening remain with the owner.
- A read-only probe of the existing local imported copy found matching source
  and reader segment text, but only 174 of 175 chunks validated. One stored
  chunk had a space inserted after a comma by the production long-sentence
  chunker; its source span correctly pointed to text without that space. The
  all-chunk coverage check therefore rejected targets anywhere in the book.
  No private title, prose, IDs or audio are added to tracked fixtures or logs.
- Extend the existing shared locator match normalization to the exact clause
  separator vocabulary used by `splitLongSentence`, alongside its existing
  sentence-terminator compatibility. Ignore only post-boundary spacing introduced
  by chunk joining. Preserve word spacing, punctuation, UTF-16 source offsets,
  exact-first lookup, ordered recovery and full coverage checks. Do not re-chunk,
  rewrite or bypass validation of imported audio.
- Runtime seam: `src/tts/alignment/domTextSegments.ts`; both highlighting and
  verified target preparation already use it. No new integration surface,
  dependency, native command, persisted schema or cache identity is introduced.
  Keep this compatibility fix if play-anywhere is removed, because highlighting
  uses the same lookup. The earlier stable-viewer correction remains included.
- Synthetic production-chunker regressions cover all nine clause separators,
  durable spans and legacy no-span recovery, preserved IDs/text/hashes, and
  rejection of changed words or punctuation. Browser controls now validate
  captures through real source preparation instead of treating capture alone
  as success. Coordinator coverage includes beginning playback followed by a
  later viewport target in a book containing the spacing mismatch.
- The read-only local probe now recovers all 175 chunks and verifies 700 boundary
  points across durable and no-span variants. Source/metadata bytes and canonical
  chunk objects remain unchanged. This establishes mapping only, not audible or
  WebKit acceptance.
- Checks: **552 passed, 1 existing skip across 79 frontend files**; full ESLint,
  TypeScript, locale parity, production Vite/Pagefind build, native release
  compilation and media-bundle verification passed. Existing Vite mixed-import/
  large-chunk warnings remain. AppImage packaging completed successfully.
- Rebuilt local artifact:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-import-fix.AppImage`;
  SHA-256 `b5424b3d5600f458095e46d29cff1aa8023174d54682b50ae5773a403128a373`.
  This is base `5c7802b` plus both uncommitted corrections. The neighboring
  `import-fix-build.json` records exact code/test patch and runtime-source hashes.
  Earlier named artifacts are preserved; the passage-fix build does not contain
  the clause-spacing correction.
- Owner retest: launch the new import-fix AppImage normally, reopen the existing
  imported book, and retry the top Play from here icon. Then use Play from
  beginning, scroll away and invoke the browse Play from here action; verify the
  containing saved chunk begins and continues in order. No audio regeneration or
  reimport is required. Also retry Play from selection. Keep remaining device,
  accessibility and performance gates open.
- Owner subsequently requested committing both corrections together. The code
  patch still matches the recorded import-fix AppImage and its 552-test receipt;
  no additional runtime changes were made for the commit. Manual acceptance
  remains pending.
- Test setup cleanup: the agent-created `PapercutAcceptance` virtual null output
  was left loaded after the earlier packaging investigation. It has now been
  unloaded; the hardware speaker remained the default before and after cleanup.
  This was temporary host test setup, not an app feature or device fault. No
  Papercut source/configuration creates that output.

### 2026-10-22 — Stage 6 sentence-closer compatibility and review

- The active isolated test profile contains a different imported HTML book from
  the earlier 175-chunk probe: **1,139 chunks / 351,726 source bytes**. Read-only
  process metadata identified the running standard AppImage and its data directory;
  no app UI, playback, output routing or library data was changed.
- One chunk differs because the production sentence regex consumes a terminator
  and closer, then omits an immediately following terminator. Synthetic example:
  source `An abbreviation (etc.). Next sentence.` becomes stored narration
  `An abbreviation (etc.) Next sentence.` The source span still includes the
  omitted period. Re-chunking this local source reproduces all 1,139 saved chunk
  texts exactly; this is a known narration transformation, not a changed book.
- Extend shared matching to this exact consumed-closer behavior. Keep raw
  whitespace/display-mark adjacency significant when deciding whether punctuation
  was omitted. A context-aware gap check handles an omitted book-final terminator;
  selecting that non-narrated character alone remains unmapped. Changed words,
  decimal values and independent punctuation still reject. No saved text, IDs,
  hashes, audio bytes or format/schema are rewritten.
- Regressions cover nine sentence terminators, eleven closers, source UTF-16
  offsets/emoji, book-final punctuation and Arabic/display marks, plus actual
  reader controls/selection with source preparation. Review caught and corrected
  an introduced display-mark adjacency edge case before packaging.
- HTML diagnostics now distinguish `source-identity`, `chunk-verification` and
  `point-lookup`; reader capture failures log their own phase. Logs contain only
  stage, typed reason and counts/request metadata, never source/selection prose
  or chunk IDs. All new diagnostic calls live in feature-owned modules.
- The final read-only local probe verifies all 1,139 chunks and **4,556 boundary
  points** across durable and no-span variants. Source/metadata bytes and chunk
  objects remain unchanged. Full frontend: **576 passed, 1 existing skip in 79
  files**; ESLint, TypeScript, locale parity and whitespace checks passed.
- Production build, native release, AppImage packaging and media-bundle checks
  passed. Existing Vite mixed-import/large-chunk warnings remain. Named artifact:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-punctuation-fix.AppImage`;
  SHA-256 `e53f953c2e95681df0e2bda2319b7ef5e105fa831205878fcfd07471dafa4d57`.
  This is base `333cca52` plus the uncommitted correction/diagnostics patch.
  `punctuation-fix-build.json` records source/patch hashes and numeric probe data.
  Earlier artifacts remain preserved and lack this sentence-closer correction.
  Owner retest must use this exact named artifact with the same imported book;
  changing app versions does not require audio regeneration. The observed test
  app used a disposable data profile, so retain that profile when launching or
  import the original bundle again if the test-profile owner has removed it.
- Resolver-only Node timings, 30 samples on this host while other automated
  checks ran: cold durable p95 **133.3835 ms**, cold legacy p95 **143.1658 ms**,
  warm point lookup p95 **0.0123 ms**. Inputs were already extracted segments.
  These exclude source parsing, DOM capture, disk/audio loading, seeking and
  audible output; they establish no WebKit/device or product latency acceptance.
  Numeric receipt: local ignored `diagnosis/mapping-review-results.json` beside
  the test artifacts. Cold work is a performance concern, not a passed gate.

### Matching architecture review and recommended next work

What this change does: recover compatibility with the narration already saved in
an imported book, without altering or regenerating audio. The current design is
safe to continue owner testing, but the repeated mismatch cases and cold timings
do not justify broad release yet. Assumed load: one local reader with a book of
roughly 1,000 chunks, plus larger/mobile cases still to measure.

| Action | How the target is chosen | Matching required? |
| --- | --- | --- |
| Play from beginning | Load the authoritative saved sequence and dispatch canonical index zero. | No DOM/source-passage match. Normal saved-audio/options checks still apply. |
| Play from here | Capture the first visible narratable source point in reading order, below/above obscuring reader controls; resolve its containing saved chunk. | Yes; incidental mouse selection is ignored. |
| Play from selection | Snapshot the document-order Range start before menu focus changes; resolve its containing saved chunk. | Yes; backward selection uses the same source-order start. |
| Resume | Continue the current transport time. | No new passage match. |
| Return to audio | Reveal the playing/paused chunk in the reader. | No seek or new playback. |

Viewport and selection use the same pipeline:

1. The optional viewer adapter captures a normalized source point and a live
   snapshot guard; it neither loads nor starts audio.
2. Source preparation compares the live normalized segment list with the selected
   sanitized HTML, then validates saved source spans, actual text, ordering,
   overlap and complete source coverage. Legacy imports may recover spans by
   exact-first, ordered text lookup with bounded compatibility normalization.
3. One immutable validated index is cached for the current source/chunk revision.
   A point lookup uses binary search. Currently **one unverified chunk makes the
   complete index unavailable**, even for a trustworthy passage elsewhere.
4. The playback coordinator rechecks document/options/request validity and sends
   the original full sequence plus its canonical index to the existing player.
   The player starts at that saved chunk's beginning and continues in order.
   Stop/close/newer requests invalidate pending work; failure never falls back
   to chunk zero without an explicit beginning action.

The issue is therefore not simply “exact matching versus fuzzy matching.” Exact
source identity and word/order checks protect against wrong audio. Compatibility
with deterministic chunker punctuation changes was incomplete, and global
coverage validation magnified each mismatch into a book-wide failure.

**Should fix before broad release:**

1. **Chunker/matcher rules can drift.** The comma and consumed-closer failures
   are concrete examples of matching reconstructing narration behavior separately
   from `appendSegmentChunks`. Next, extract a small shared, pure source-to-speech
   projection/compatibility contract while preserving every existing saved chunk
   text/hash. Reuse it for generation, highlighting and target validation. This
   reduces repeated exception patches; the cost is a carefully tested shared-code
   refactor. Keep it independent of the play-anywhere folder. Do not change old
   synthesis text/cache versions to repair targeting.
2. **Cold validation can block interaction.** The measured synchronous resolver
   path takes about 133–143 ms p95 in Node before capture/audio costs. Warm binary
   lookup is cheap, so replacing the lookup algorithm is not the priority. Measure
   the packaged WebKit flow, then split cold preparation into cancellable batches
   that yield to rendering. Move pure string/index work to a feature-owned worker
   only if batching cannot meet device budgets; worker startup and copying have
   costs. No per-scroll/timeupdate whole-book work or unbounded range cache.
3. **A local mismatch currently disables every passage.** First keep truthful
   phase diagnostics and the explicit beginning fallback. Then evaluate validating
   each requested durable span independently, after source identity and global
   bounds/order/overlap checks, while preserving its original canonical index.
   This can keep verified regions usable; it adds partial-state/indexing cases
   that require tests. Keep full ordered coverage proof for legacy recovery and
   repeated-text ambiguity. Never guess the nearest phrase or silently seek zero.

Keep the current modular architecture. A generalized fuzzy-search/ranking system
would introduce confidence thresholds and risk wrong repeated passages without
solving the known deterministic transformations. Persistent per-word timestamps,
new settings, schema migrations and audio regeneration remain out of scope.

Removal remains bounded: disconnect App/manager feature hooks and optional viewer/
control capabilities; restore ordinary zero-start Read and default following;
delete `src/tts/playAnywhere/` and unused feature labels/diagnostics. Remove the
new `locatorRangeIsIgnorable` export if unused after that deletion. Retain stable
HTML node rendering and the shared punctuation/Unicode fixes used by baseline
highlighting. No saved book, cache, preference or schema rollback is required.
The Stage 7 disposable removal rehearsal must verify this, rather than assuming
that the dependency map alone proves removability.

Verdict: finish packaged listening/accessibility/timing acceptance and the shared
projection/cold-preparation work before broad release. The current corrective
patch is ready for owner retesting once its package checks complete; those gates
and the removal rehearsal remain open. This review does not authorize unrelated
refactors, automatic manual app testing or changes to saved data.

Recommended next checklist, after the owner retests this candidate:

- [x] Extract the shared pure narration compatibility contract without changing
  generated chunk texts, IDs or hashes. Acceptance: existing golden cases plus
  sentence/clause/closer/Unicode cases produce identical canonical narration;
  generation, highlighting and targeting use the same rules; the shared helper
  imports no feature module and baseline tests run without the feature adapter.
- [ ] Measure cold/warm packaged WebKit capture-to-dispatch and audible timing,
  including first action, large imports and mobile. Acceptance: recorded samples
  separate capture, validation, audio preparation and audible start; no Node-only
  number is presented as device acceptance.
- [x] Implement cooperative cold-preparation batches (owner authorized before
  packaged timing acceptance); retain the separate device timing gate. Acceptance: loading can paint and Stop/close/newer intent
  can invalidate work between batches; original source/chunk identities and warm
  cache lookup stay intact. Keep any worker/batching lifecycle feature-owned.
- [ ] Decide whether durable span verification may be local to the requested
  chunk. Acceptance: source identity, global bounds/order/overlap and original
  canonical indices remain verified; unrelated invalid text cannot cause a wrong
  seek; legacy repeated/ambiguous text still requires complete ordered proof.
  Update the conservative all-or-nothing contract before implementing this change.
- [ ] Complete the existing Stage 7 disposable removal rehearsal. Acceptance:
  ordinary beginning/resume/highlighting, Find and bookmarks still work after
  optional capture/target/follow/control wiring is disconnected; no user-data
  deletion, schema migration, cache regeneration or worker/process leak.

### 2026-10-22 — Recommendations 1 and 2: shared rules and cooperative preparation

The preceding sentence-closer and diagnostic fixes are committed as `af9f429`
(`fix(reader): match saved sentence-closer punctuation`). This follow-up keeps
complete ordered verification for both durable and recovered spans. Recommendation
3 (partial durable validation) is a separate design decision, not implemented.

Implementation checklist and acceptance:

- [x] Move sentence/closer/clause vocabulary into the pure
  `alignment/narrationText.ts` contract. Generation calls its sentence/clause
  splitters; highlighting and target verification use its matching projection.
  Projection walks the same raw sentence matches as generation, retaining an
  output-unit-to-original-UTF-16-offset map. It does not independently reconstruct
  which terminal the chunker omitted. Unicode compatibility and exact-first
  ordered recovery remain. Independent final punctuation still requires proof;
  an end-of-index regression prevents falsely treating the last period as a gap.
- [x] Preserve all three shipped chunk profiles. A pre-refactor golden corpus
  covers sentence/clause punctuation, twelve closers, emoji, Arabic/display marks,
  decimals, abbreviations, long input and leading punctuation. Its canonical
  chunk texts and source spans retain SHA-256
  `479519f91412708ae305baea2a2393a26f36adf245fdb0823e2ebda1acd7f38a`.
  Compatibility tests compare source and generated narration without conflating
  ordinary word boundaries or decimal values. No saved IDs, hashes, audio or
  manifests are rewritten.
- [x] Keep baseline APIs synchronous. `finishNarrationWork` drains pure generator
  steps for highlighting and existing callers; only play-anywhere schedules them.
  Common alignment/generation imports no feature, timer, React hook or player.
- [x] Own asynchronous work in `playAnywhere/preparationWork.ts`. Defer the first
  pump; process at most 128 yielded steps or an 8 ms time budget per pump, then
  schedule the next task. Index projection yields every 2,048 source units;
  segment comparison, legacy recovery and validation yield between units/chunks.
  This is a **cooperative scheduling budget, not an 8 ms maximum task guarantee**:
  DOMParser/source extraction, a single string operation, resolver construction
  and one validation step remain synchronous. A worker is deferred until actual
  packaged measurements identify a remaining stall worth that complexity.
- [x] Extend the optional resolution seam to
  `resolve(documentUrl, capture, signal?: AbortSignal)`. The playback coordinator
  aborts prior preparation for a newer action (including beginning), Stop and
  scope disposal. Source/view guards are checked between steps and after awaits;
  document/options changes dispose the existing hook scope. Native manifest/page
  reads cannot be physically aborted here; late replies remain guarded.
- [x] Coalesce compatible cold requests and publish only a complete validated
  cache. An older job cannot clear/overwrite newer pending work. Abort/disposal
  clears timers/listeners and closes the iterator; partial state stays private.
  Warm requests reuse the existing resolver with no batching timer. Cache ownership
  remains per document/content/chunks/view snapshot, with the existing bounded PDF
  page cache. No persistent cache, new setting, schema, native command or dependency.
- [x] Log only batch count, maximum observed batch time and preparation elapsed
  time on completion through existing bounded diagnostics. Rejection phases keep
  typed reasons/counts. No source/selection text or chunk IDs are logged.
- [x] Cover cancellation/coalescing/retry/warm reuse and exceptions, including
  disposal/view invalidation between batches and newer-action signal abortion.
  An isolated Chromium fixture uses 1,200 synthetic paragraphs with legacy saved
  chunks: a frame and another event run before completion; cancellation returns
  stale; successful lookup retains canonical indices and saved object identity.
- [x] Re-run the read-only imported-book regression: all **1,139 chunks / 4,556
  durable and legacy boundary checks pass**, source/metadata bytes and chunks
  unchanged. One Node/jsdom observation: durable cold 167.57 ms with 17 heartbeat
  turns; legacy cold 143.74 ms with 26 turns; warm 0.095/0.006 ms. These single
  observations include source parsing, exclude capture/audio/seek/audible output,
  are not directly comparable with the prior resolver-only samples, and establish
  no latency improvement or device acceptance. The largest observed batch was
  105.89 ms (durable) / 68.96 ms (legacy), underscoring the synchronous parsing
  limitation. Numeric local receipt: `diagnosis/responsive-preparation-results.json`.
- [x] Frontend regression: **593 passed, 1 existing skip, 82 files**; lint,
  TypeScript, locale parity and production Vite/Pagefind build pass. Existing
  mixed-import/large-bundle warnings remain.
- [x] Native release, Linux AppImage packaging and GStreamer media verification
  pass. Owner candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-responsive-preparation.AppImage`,
  SHA-256 `8792b938d0da763e2aeda8e79502b4d3b0de667808c58af99c6cf490813239b4`.
  Base `af9f429` plus this follow-up; adjacent `responsive-preparation-build.json`
  records provenance. Older named artifacts are preserved. Close the old app,
  launch this exact candidate normally with the same book/profile and existing
  audio. No app UI/playback/output routing was controlled by the agent.
- [ ] Owner packaged WebKit acceptance: fresh and warm selection/viewport starts
  on the same imported bundle, correct canonical chunk beginning and continued
  sequence, no opening blip, responsive pending/Stop/close/newer intent, no late
  playback, ordinary beginning/resume/highlighting and no source/audio rewrite.
  Record capture, source validation, audio loading/seek and audible timing
  separately, including first action and larger books. Keyboard/screen-reader,
  mobile and the Stage 7 disposable removal rehearsal remain pending.

Removal/debugging recipe for this follow-up: abort/dispose the optional preparation
scope, disconnect manager/App target wiring, delete `playAnywhere/` (including the
scheduler and its browser fixture), and restore baseline zero-start/default-follow
controls as already listed above. Keep the common narration grammar, synchronous
wrappers and highlighting compatibility. Remove `prepareMatchIndex` and other
feature-only exports if unused after deletion; generator internals may still serve
synchronous wrappers. There is no worker/process, saved state or migration to undo.
A failed target still reports source identity, chunk verification or point lookup;
batch metrics distinguish cold preparation from cached lookup. One incompatible
chunk still disables the full map; this conservative rule is unchanged.

### 2026-10-22 — Option 1: inline Return and passage popover

The preceding shared narration/cancellable preparation work is committed as
`039d11d` (`refactor(reader): share narration rules and yield target preparation`).
The owner approved the compact UI after reporting that targeting works better.
This follow-up changes presentation and capture timing, preserving source matching,
canonical chunks, audio queues, native behavior, schemas and persisted data.

Implementation and acceptance checklist:

- [x] Remove the separate `.audio-browse-actions` surface. Show one labeled,
  icon-prefixed **Return to audio** button within `.audio-floating-playback`
  while the existing browsing state is active. Return only reveals/restores
  following; paused/playing state and audio time are unchanged. Previous, next,
  pause/resume, speed and Stop keep their meanings. At narrow widths/large text,
  controls may wrap inside the same surface; no forced single-row layout.
- [x] Add **Play from here** to the existing popover alongside **Play from
  selection** and **Play from beginning**. Use the existing inline SVG renderer,
  visible labels, theme colors, command focus/hover/disabled states and control
  radii. Here spans the group; selection/beginning share a row when they fit and
  stack when available width, text size or translations require it. These commands
  no longer reuse the word-highlight toggle's pill styling.
- [x] Keep word highlighting as its existing real toggle, visually separate from
  passage commands. Reuse existing labels/guidance in all eight locales; no new
  dependency, locale key, setting or global shortcut.
- [x] Extend only the optional UI seam `onSnapshotSelection?: () => boolean`.
  The reader hook still stores at most one guarded snapshot; its return value
  reports whether a selection was captured (including reuse after native focus
  collapse). Controls store availability for this popover session. Without a
  capture, disable selection and expose the existing localized no-selection hint
  through visible text and `aria-describedby`. Availability is not proof of a
  resolvable audio chunk; existing action-time source/intent guards still decide.
- [x] Focus the first enabled popover command. Escape/dismissal and actions retain
  trigger-focus behavior. Removing a focused Return transfers focus to surviving
  transport. Native selection remains untouched. Selection consumes its saved
  snapshot before popover-effect cleanup; synchronous early closure was caught
  by pointer/keyboard browser regressions and corrected before packaging.
- [x] Close the popover synchronously before viewport capture. The feature reader
  hook commits pending/capture/feedback UI before measuring source bounds, so
  old error notices cannot cause a false stale target on retry. Selection retains
  its original pre-focus source snapshot. No additional frame/timer lifecycle.
- [x] Fit popover height to the space above the dock and visible viewport. Own the
  ResizeObserver and window/visualViewport listeners only while open; remove them
  on close/unmount. Keep overflow reachable under large text, including the jump
  list; preserve virtualized 44 CSS-pixel rows without oversized vertical padding.
  Prior notices are visually clipped while the popover is open; live status stays
  mounted and new action failures reappear after the command closes it. This
  prevents old feedback from consuming most of the remaining reader height.
- [x] Tests cover 320 px English, 620 px English, large-text dark Arabic RTL and
  large-text French desktop. All commands retain at least 44 CSS-pixel height;
  the two secondary commands share a row at 620 px and stack in the narrow
  large-text case. Popover stays on screen; scrolled commands/list remain
  reachable; disabled selection, Escape/focus return, inline Return, missing
  feature capability and real viewport/selection targeting retain coverage.
  Synthetic screenshots were visually inspected; local ignored captures are
  under `src-tauri/target/play-anywhere-acceptance/ui-review/`.
- [x] Full frontend **598 passed, 1 existing skip, 82 files**. Lint, TypeScript,
  eight-locale parity and production Vite/Pagefind build pass. Existing mixed-import
  and large-bundle warnings remain. No app UI, playback or output routing was
  controlled; automated browser fixtures are isolated from the owner's profile.
- [x] Final native release/AppImage packaging passed. Candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-compact-controls.AppImage`;
  SHA-256 `48dec9b4eb39063925f46fe041bdb126ce548e5213aad97a3b0e54f4288163a9`. Adjacent
  `compact-controls-build.json` records base `039d11d` and runtime source hashes.
  Older named candidates are preserved. New UI code was expanded into readable
  blocks/imports following owner feedback, then targeted tests/build were rerun.
  Function-level notes explain selection/menu lifetime, chrome-before-capture
  timing, Return/follow intent and large-text popover fitting. Documentation-only
  comments added after packaging emit byte-identical frontend assets; receipt
  records both source hashes. GStreamer media verification also passes.
- [ ] Owner Linux candidate acceptance: browse and use inline Return without a
  seek; use all three popover commands on the same imported book; mouse/keyboard
  selection survives focus; Resume stays at its exact time; retry after a prior
  error; check narrow/large-text layouts and Stop during preparation. Audible
  correctness, actual WebKit/screen reader/mobile and release/removal gates remain
  pending. Estimated jump-list timestamps remain a separate follow-up.

Removal remains through the existing optional `playAnywhere` prop and App/manager
composition seams. Delete feature hooks and passage/Return controls/styles/glyphs
when unused; restore ordinary idle play/default follow. The general popover fit,
focus and virtual-row layout fixes may remain with the baseline audio menu. There
is no native/player/matching change or user-data rollback in this UI follow-up.

### 2026-10-22 — Reader shortcut and chunk-first popover

This supersedes the preceding popover layout after owner testing showed that
passage commands and highlighting left too little room for the chunk list.
Only the two severity-3 recommendations are implemented in this pass.

Integration and acceptance checklist:

- [x] Add an optional `floatingControls?: ReactNode` presentation slot to
  `DocumentViewer`, rendered beside Bookmark/Top using the existing scroll
  threshold (including the PDF bookmark API). The viewer does not import the
  new audio shortcut or decide which playback action to run.
- [x] App supplies feature-owned `ReaderPassageAction` only when the selected
  document has playable saved audio and the floating player is absent. The
  paused player counts as present, so Resume keeps its exact-time behavior.
  Scrolling while stopped exposes **Play from here** without returning to the
  header. A nonempty reader selection changes this to **Play from selection**.
  The initial label is superseded by the 2026-10-23 session presentation below;
  no permanent second playback strip is introduced.
- [x] The shortcut listens to selection changes only while mounted. Availability
  checks inspect the current range, not a book-sized audio map. Pointer input
  snapshots before control focus; keyboard Tab reuses the existing selection
  intent binder. Focus-induced native selection collapse preserves the advertised
  action. Stale selection reports the existing failure rather than silently
  playing a different viewport. Action-time source/chunk checks remain authoritative.
  A scoped ResizeObserver reserves the shortcut's actual height above the rail
  for bookmark feedback, including wrapped translations; unmount disconnects it
  and removes its CSS variable. Unmount removes listeners; starting playback
  transfers focused shortcut control to the resulting player when possible. The binder recognizes the new control
  as audio input rather than invalidating its snapshot.
- [x] A loaded chunk menu opens on **Chunks**, showing count and the virtualized
  list. One compact Chunks/Options view selector replaces the permanently stacked
  commands, Playback header and Jump To subheader. **Options** contains the three
  labeled passage commands and Word Highlight. With no loaded summaries, the
  menu opens directly on Options. Reopening a loaded menu defaults to Chunks.
  View buttons use normal Tab/Enter/Space operation and `aria-pressed`; they do
  not claim tab semantics requiring a different keyboard model.
- [x] Let the list fill the available panel height rather than retain its old
  three-row flex basis. Preserve 44px virtual rows and existing viewport fit.
  Synthetic 320px/620px English fixtures allocate over 75% of the popover to the
  list; 32px-root Arabic/French fixtures retain over 50%, with reachable content.
  These are fixture acceptance checks, not a claim of universal row counts.
- [x] Preserve menu pre-focus selection snapshots across view switching, synchronous
  closure before viewport capture, Escape/trigger focus, Return/follow, canonical
  chunk-start/full-book playback and existing Stop/cancellation guards. No source
  matching, native player, cache identity, persistence or generation changes.
- [x] Add localized Chunks/Options labels in all eight locales. No dependencies,
  flags, settings or global event bus. Add regressions for view reset/dispatch,
  missing selection, backward selection and pointer/keyboard collapse in the
  scrolled idle reader. Isolated browser fixtures use synthetic documents only.
- [x] Frozen-source regression suite: **605 passed, 1 existing skip, 83 files**.
  Lint, TypeScript, eight-locale validation and production frontend build pass.
  Focused browser/unit checks also cover shortcut-to-player focus and bookmark
  feedback spacing. Synthetic chunk-list and shortcut screenshots were inspected.
- [x] Native release and AppImage packaging pass. Named candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-chunks-first.AppImage`;
  SHA-256 `aeefca480d5c397c347632738ab3e34ce772a0012d183d2e970b132d9bb63ce6`. Adjacent `chunks-first-build.json` records
  base commit, all runtime source hashes, validation and manual acceptance limits.
  The package copy matches the final build byte for byte; older named candidates
  remain available. GStreamer media-bundle verification passes.
- [ ] Owner acceptance on the new Linux AppImage: stopped + scroll exposes
  Bookmark/Top and the contextual play action; select text in either direction
  and use that action without scrolling up; verify containing-chunk start/full
  continuation; playing/paused menu opens on the larger list; Options exposes
  all commands and highlighting; Escape returns focus; recheck large text/RTL.

Removal: remove App's `ReaderPassageAction` import and `floatingControls` value,
then delete `ReaderPassageAction.tsx`, its CSS/test and binder selector additions.
The unused generic slot can be deleted or retained. All target preparation and
selection coordination stay behind the existing feature seam. The Chunks/Options
presentation can remain as a baseline audio-menu improvement; feature passage
commands still disappear when the optional `playAnywhere` prop is omitted. No
schema migration or user-data rollback is required.

Next, separately: stop list recentering after deliberate user scrolling, compact
Return styling, and integrate themed pending/error feedback with the existing
spinner. The very large-text narrow player can still wrap and reduce list height;
Return sizing is deliberately deferred. Estimated timestamps and Stage 6/7
platform, listening, assistive-technology, performance and removal gates remain
open. Do not conflate these follow-ups with the two severity-3 fixes.

### 2026-10-22 — Stable browsing, compact Return and integrated feedback

The prior reader shortcut/chunk-first pass is committed as `a7dacd1`
(`feat(reader): expose passage playback and prioritize chunk browsing`). The owner
explicitly deferred smoke tests until this next pass. That earlier acceptance
checklist remains open; automated checks do not substitute for audible/device
acceptance. This pass implements the next three severity-2 UI recommendations.

Implementation and acceptance checklist:

- [x] Keep automatic list centering until the reader deliberately browses. Wheel,
  pointer (including scrollbar/touch input) and focus entering chunk rows mark
  this popover session as browsing. Programmatic `scroll` events update virtual
  rows and saved position but do not themselves suspend following. Playback
  updates thereafter preserve the reader's list position. Options -> Chunks
  restores that position; closing/reopening creates a fresh centering session.
  No timers, document-level listeners or persistent preferences are added.
- [x] Give Return the ordinary tonal transport styling. Wider windows retain a
  short localized **Return** label; <=480px windows show its icon at a 44px target
  for the ordinary 16px root. Both forms retain the full localized accessible
  name and title **Return to audio**. The fully labeled command also appears in
  Options while browsing. Existing optional capability guards apply to both;
  Return only reveals/restores following and does not seek, resume or restart.
  Wrapping remains allowed at narrow widths/large text.
- [x] Group transport and transient status under `.audio-playback-surface`, using
  existing surface/control/warning tokens. Remove the separate notice shadow/card
  treatment. Native loading and passage preparation show the existing decorative
  spinner beside explanatory text, with the existing reduced-motion behavior.
  Fallback loading text uses the existing translated background-playback label.
- [x] Retain one mounted polite atomic status region; loading is not communicated
  through animation alone, and status updates do not take focus. Errors/failures
  retain their explanatory/recovery copy and replace the spinner. Empty status
  leaves no visible notice. Preserve the existing visually clipped notice during
  source capture/open popover, so asynchronous feedback cannot destabilize the
  viewport capture stamp or crowd the Options panel. Feedback reappears when
  commands close the popover, including action failures.
- [x] Preserve synchronous chrome-before-capture commits and pre-focus selection
  snapshots. Existing pointer/keyboard, old-error retry and imported punctuation
  browser regressions continue to cover viewport and selection playback.
- [x] New component checks cover wheel/pointer/keyboard browsing, automatic center
  before browsing, Options round-trip/restored position, recenter on reopening,
  compact/full Return dispatch, one live region and spinner/error transitions.
  Browser fixtures cover actual scroll stability, narrow icon-only and wider
  labeled Return, light/dark tonal styling, and visible recovery feedback.
- [x] Frozen-source suite: **613 passed, 1 existing skip, 83 files**. Lint,
  TypeScript and eight-locale validation pass. The full suite includes real
  Chromium scroll and feedback regressions; screenshots were visually inspected.
- [x] Production frontend, native release, AppImage packaging and GStreamer
  media checks pass. Candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-control-refinements.AppImage`;
  SHA-256 `2f1cb15c72feebb722f77f55359ed5570187640ce2756532017317c938f288af`. Adjacent
  `control-refinements-build.json` records base `a7dacd18`, ten runtime source
  hashes, validation and pending manual acceptance. The named copy matches the
  packaged artifact byte for byte; previous named candidates are preserved.
  Final 21 browser checks also pass after readability-only fixture cleanup.
- [ ] Owner combined smoke acceptance on the new candidate: all previous stopped
  scroll/selection/chunk-start checks, then browse the list while chunks advance,
  round-trip through Options and reopen; use both Return forms; check loading
  text/spinner, recovery after errors, large text/narrow layouts and keyboard focus.
  Actual Linux WebKit/listening/AT/mobile and Stage 6/7 gates remain pending.

Integration/removal: all new behavior is within `AudioControls.tsx`/CSS plus eight
short-label translations and focused tests. Return and its Options command remain
behind optional `playAnywhere` callbacks; remove those controls/styles/translations
with the feature. The generic list-browsing and shared status surface can remain
with the baseline player. No matching, queue, source API, saved-audio identity,
reader persistence, native code, migrations or dependencies change in this pass.

Next: owner smoke results, then a documented feature-removal rehearsal and remaining
performance/accessibility/platform acceptance. Estimated chunk timestamps remain
a separate correctness follow-up; they are still estimates, not measured offsets.

### 2026-10-22 — Narrow toolbar and automated removal/acceptance rehearsal

The preceding refinements are committed as `9e099e89`
(`fix(reader): keep audio navigation steady and compact`). The owner again deferred
smoke testing and requested a next pass, including review of the isolated Return
row in the 320px capture. All owner/device/listening acceptance remains open.

UX decision and implementation:

- [x] Treat the orphan Return row as a minor usability/layout problem: it consumes
  reading height and separates a contextual navigation action from its transport
  group. Do not solve it by shrinking controls or changing Previous/Next meanings.
- [x] At <=480px, move playback speed from the transport into the existing Options
  view; reuse its existing cycling callback and translated speed labels. Keep
  Previous, Play/Pause, Next, Menu, Stop and contextual Return directly available.
  Six 44px targets with 3px gaps/padding fit the tested 320px/16px-root toolbar.
  Speed stays in its normal location above 480px. Stop stays directly reachable.
  When Return is absent, five targets remain; no automatic button-role swaps.
- [x] Preserve wrapping at enlarged root text; a one-row rule at every zoom level
  would risk clipping or undersized targets. Fixing a lone action at ordinary
  320px does not justify suppressing reflow. Remove a duplicate narrow-surface
  CSS block encountered in the existing media rule; no global style migration.
- [x] English/light and Arabic/dark browser checks prove six aligned targets,
  >=44px dimensions and on-screen bounds at 320px; speed remains reachable and
  dispatches the same1.25x callback from Options. The Chromium accessibility tree
  exposes named buttons and the full Return name. Enabled transport icon contrast
  meets 3:1 in these fixtures. Existing large-text/RTL, focus, feedback, capture and
  selection regressions remain. This is targeted automation, not WCAG certification.

Rationale: [NN/g contextual-menu guidance](https://www.nngroup.com/articles/contextual-menus-guidelines/)
places secondary actions in clearly labeled menus while warning about discovery
cost. [Apple Books](https://support.apple.com/en-ca/guide/iphone/iphac1971248/ios)
uses compact and expanded playback surfaces. We apply that hierarchy to Papercut's
speed control; this is our design choice, not a claim that Apple uses this exact
320px layout. [WCAG reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html)
requires retained functionality at 320 CSS pixels, not a mandatory single row;
[WCAG target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)
sets a24px AA minimum with exceptions. 44px is our touch design target here.

Automated removal rehearsal (partial Stage 7):

- [x] Create a disposable `/tmp/papercut-removal-*` source copy; do not reset,
  stash, modify or delete the owner's checkout, Git history, preferences or data.
  Dependencies are symlinked for checks; TypeScript build caches are redirected
  to the copy. Copy the three public notice Markdown imports explicitly.
- [x] Delete `src/tts/playAnywhere/` from that copy. Disconnect five runtime files:
  App (reader hook/shortcut/optional UI inputs), useAudiobookManager (prepared-target
  hooks; restore the existing saved-chunk speak action), DocumentViewer (feature
  binders/viewport provider), HtmlViewer and PdfViewer (capture setup imports/effects).
  Clear the coordinator's cancel wrappers/dependencies; keep the player's own
  transport cancellation. Keep shared narration/alignment, highlighting and data
  modules. Adjust two viewer test files to remove feature-only capability tests.
- [x] Reproduce that procedure in a fresh copy. Baseline TypeScript passes;
  **61 tests across 10 files** pass for controls, player, viewers, bookmarks,
  highlighting, narration identity/normalization and saved-audiobook storage.
  A production Vite bundle with static public assets excluded contains **zero
  feature-owned modules** across 566 module entries. This structural bundle is
  not a packaged baseline application or listening acceptance result.
- [x] Save the disconnect patch, exact seams, scripts and manifest under ignored
  `src-tauri/target/play-anywhere-acceptance/removal-rehearsal/`. After applying
  `disconnect.patch` in a disposable source copy, remove the feature directory;
  never apply this rehearsal to the owner checkout. Generic optional types and
  baseline audio presentation can remain. No migrations, source/audio regeneration
  or user-data deletion are needed by this source-only disconnection.
- [x] Verify the normal feature checkout remains intact and rerun its regression
  checks. There was no live patch to restore. Native/user-profile files are not
  copied or accessed by the rehearsal.
- [ ] Packaged baseline playback, actual screen-reader/native/mobile checks and
  release decision remain pending. Stage 7 is not fully accepted by this rehearsal.

Performance evidence:

The existing synthetic preparation fixture now records 30 warm resolver-only samples,
post-yield browser long-task entries, cooperative batch metrics and cold completion.
A task boundary separates fixture generation from the observed interval. It covers
1,200 legacy chunks/360,090 source bytes, unchanged canonical identity, a rendered
frame and another event before completion, and cancellation. Results are recorded
in `diagnosis/preparation-30-sample.json` beside the acceptance artifacts. These
measurements exclude DOM capture, file reads, track stitching, native seeks and
audible playback; a zero rounded timer reading is not an instantaneous operation.
No wall-clock assertion or whole-product performance pass is inferred from them.

- [x] Normal feature suite: **616 passed, 1 existing skip, 83 files**. Lint,
  TypeScript, eight-locale validation, production frontend/native release and
  AppImage packaging pass. Final focus-order checks are included. Fixed synthetic
  evidence in `diagnosis/narrow-preparation-measurement.json`: cold **453.2ms**,
  **30 warm samples/p95 0.1ms**, maximum cooperative batch **8.3ms** and no
  long-task entries observed in that interval. Browser: HeadlessChrome 155 on
  Linux x86_64, reporting 8 logical processors; host metadata is in the rehearsal
  manifest. These are observations, not reference-device acceptance.
- [x] New named package:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-narrow-controls.AppImage`;
  SHA-256 `f0664863400d272185d34632c960603883083c6f7fcb4f3ee6cf3b85ea772dcd`. The copy is executable and
  byte-identical to the final bundle. Adjacent `narrow-controls-build.json` records
  base `9e099e89`, runtime/test hashes, automated evidence and open gates. Earlier
  named candidates are preserved. GStreamer media-bundle verification passes.
- [ ] Owner combined smoke: stopped scroll/selection, canonical chunk start,
  six-action320px toolbar, speed in Options, Return, list browsing, pending/error
  feedback, large text, keyboard and Stop/cancellation. Prior device matrices and
  request-to-audible budgets remain pending while smoke tests are deferred.

Removal scope for this pass: no new production coordinator, setting, data schema,
dependency or native code. Narrow speed placement and layout can remain in the
baseline player; deleting Return still follows the existing optional UI seam.
Next is owner smoke/device calibration and the remaining Stage 6/7 acceptance,
not an automatic merge or release. Estimated chunk-list timestamps remain separate.

### 2026-10-23 — Header reflow and session-aware reader presentation

The later capsule follow-up supersedes the session-dependent label and removes
its now-unused state. This section records the preceding candidate's decisions.

Owner clarification: the initial reader shortcut shows only Play for each new
document-open session. This is presentation only: availability, viewport/selection
targeting, canonical chunk start, Resume and Return semantics stay unchanged.

Implementation and acceptance checklist:

- [x] Protect Back's intrinsic width at intermediate widths by constraining the
  controls column instead. Allow the header's audio row to wrap at enlarged text;
  do not hide saved versions or use z-index to mask overlapping targets.
- [x] Extend the feature-owned reader hook with observational `playing` input and
  transient `hasPlayed` output. Successful passage acknowledgement or the existing
  player's playing state records a start. Pending/failed preparation does not.
  Stop/paused state retains this flag; changing/closing the document clears it,
  including closing and reopening the same URL. Nothing is persisted.
- [x] App passes only this presentation flag to `ReaderPassageAction`. Before the
  first start, match Bookmark's square size and tonal icon styling; keep the full
  target-specific accessible name/title, pre-focus selection snapshot and spinner.
  After starting, the next idle/stopped shortcut has its passage label. During
  playing/paused playback, existing transport and labeled Options commands remain
  the access path; no new active-playback floating strip or dialog is introduced.
- [x] Add decorative list/sliders icons before the existing Chunks/Options labels.
  Preserve the view-selector button semantics, count, focus and virtualized list.
- [x] Group Word Highlight and speed in a wrapping settings row with matching
  control heights/radii. Speed shows only its current rate, e.g. `1x`; the existing
  translated instruction stays in its title and full accessible name. Rate is
  available in Options at all widths; narrow transport still hides its duplicate.
  Keep the Beta disclosure, pressed state and unchanged cycling/toggle callbacks.
- [x] Regression coverage: complete DocumentViewer header with saved-audio/play/
  menu/save and reader controls at 320/453/620/900px, including 32px-root text and
  Arabic RTL. Check pairwise nonoverlap, hit targets, viewport bounds and Back
  dispatch. App Settings is a size-matched presentation stub, not a native test.
  Compact Options checks cover icons, rate label, >=44px settings and side-by-side
  English/light and Arabic/dark at 320px; enlarged translations may wrap. Existing
  browser selection/focus/chunk-start and baseline menu coverage remains in place.
- [x] Final automated validation: **625 passed, 1 existing skip, 83 files**; lint,
  TypeScript, eight-locale validation, production frontend/native release,
  AppImage packaging and bundled media verification pass. New candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-session-controls.AppImage`.
  SHA-256 `4ccc77dd65ecfb7273f100954d46d3301a98a5a64dee1f5548f6aa5baedebd0d`.
  The executable copy matches the finished bundle; frozen runtime/test hashes
  match the source. Adjacent `session-controls-build.json` records provenance
  against base `9e099e89` with this and the previous pass still uncommitted.
- [ ] Owner smoke: fresh open -> scroll -> square Play; select text -> same icon
  with selection action; successful start -> pause/browse/Options -> labeled Here;
  Stop -> labeled idle shortcut; close/reopen -> square Play again. Test Back and
  saved versions at the previously overlapping width, rate/highlight callbacks,
  large text/RTL, keyboard focus and preparation errors. Manual acceptance remains
  deferred to the owner; no user app, playback or audio routing was controlled.

Removal/debug boundary: `hasPlayed` belongs only to `usePlayAnywhereReader`, with
one observational App input and one shortcut prop. Removing the existing feature
hook/shortcut seams removes this state as well; no player, matching, storage,
schema or native change is needed. Header reflow, menu icons and ordinary playback
settings can remain with the baseline reader. Diagnose presentation in the hook
and shortcut; source/audio failures remain in the existing preparation/coordinator.

Rationale: [Readwise Reader's documented Play and jump/return behavior](https://docs.readwise.io/reader/docs/faqs/text-to-speech)
supports distinguishing initial playback from relocating active playback. We do
not copy its transport-button replacement. [Apple Books](https://support.apple.com/en-ca/guide/iphone/iphac1971248/ios)
uses a current-rate speed control. Keep unfamiliar passage commands labeled per
[NN/g icon guidance](https://www.nngroup.com/articles/icon-usability/); initial Play
remains named for assistive technology. Severity estimates from screenshot/code
review were 3 for header overlap, 2 for shortcut/settings crowding and 1 for missing
view icons, not user-study measurements or a standards certification.

#### Remaining work after the combined UI/acceptance-evidence commit

The implemented UI and automated checks are ready for consolidated owner testing;
there is no additional mandatory UI implementation step before that smoke pass.
The owner has deferred manual testing until the end. Do not infer acceptance from
completed tests or from the structural removal rehearsal.

- [ ] Owner Linux AppImage smoke: use the combined checklist above, including
  imported HTML bundles, selection, canonical chunk start/continuation, exact-time
  Resume, Return without seeking, Stop/cancellation, narrow header and settings.
  Consolidate earlier candidate retests into this latest candidate, not one smoke
  run per historical AppImage.
- [ ] Finish the device/format and accessibility matrix: PDF/OCR, supported mobile
  and desktop WebViews, real keyboard/screen reader, touch selection/fullscreen,
  RTL/enlarged text and applicable background/lock-screen controls. Record which
  configurations were tested, failed or remain untested.
- [ ] Measure packaged cold/warm action-to-dispatch and audible start on reference
  devices, including large imports. Existing synthetic resolver measurements do
  not include source capture, disk, native playback or audible output.
- [ ] Complete packaged baseline playback after feature disconnection, including
  existing reading/highlighting/Find/bookmarks and unchanged saved data. The
  disposable frontend compile, 61 baseline tests and zero-feature-module bundle
  are completed structural evidence, not listening acceptance.
- [ ] Resolve any findings, then record the accepted commit/build, device results
  and release/removal decision for Stages 6/7. No automatic merge or release.

Separate follow-ups, not prerequisites for this smoke pass: decide whether durable
span validation should permit individually verified regions of a partially
mismatched book (recommendation 3), and address the chunk list's estimated time
labels. The conservative full-map proof and original canonical chunk indices
remain unchanged. The DX linting branch also remains separate from this feature.

### 2026-10-23 — Reader capsule and cohesive Options

Owner authorized the screenshot-review recommendations: compact reader tools,
equal passage commands, a Word Highlight switch and ordinary tonal popover
styling. This supersedes the preceding session-dependent label expansion.

Implementation and acceptance checklist:

- [x] Replace the three separately elevated reader buttons with one restrained,
  rounded horizontal capsule. Play, Bookmark and Top retain individual >=44px
  targets at ordinary text size, names, focus and existing actions. Keep the
  active-bookmark shape/state feedback; Top no longer has primary-action fill
  inside this capsule. Other uses of ScrollTopButton retain their existing style.
  Allow wrapping when enlarged text cannot fit all targets; never shrink them to
  force a row. Saved audio still enables the idle shortcut without starting it.
- [x] Keep Play icon-only throughout the open session and after Stop, with the
  current viewport/selection accessible name and title. Delete `hasPlayed`, its
  observational `playing` input, App wiring and obsolete state-specific tests.
  Selection snapshots, stale-selection rejection, pending spinner, focus transfer
  and active/paused shortcut availability stay unchanged. No new mode or setting.
- [x] Give Here/Selection and Return/Beginning equal grid cells where labels fit.
  Retain one-column reflow at narrow widths/enlarged text, full labels, icons,
  pending/selection disabled state and existing dispatch callbacks. Return remains
  conditional. Add a short selection hint in all eight locales; retain the more
  detailed action-time no-selection error rather than conflating copy contracts.
- [x] Replace Word Highlight's pressed button with a labeled native checkbox
  exposed as a switch. Stable accessible label, checked state, Beta description,
  whole-label pointer activation, focus and the existing immediate callback stay
  together; no switch is nested in a button. Speed still displays its current rate
  and reuses the existing callback; large text may wrap settings into two rows.
- [x] Extract the existing Settings switch visuals into baseline
  `components/AppSwitch/AppSwitch.css` and reuse them in Settings, semantic-search
  setup and audio Options. The extraction changes no existing setting behavior.
  Keep RTL, checked/thumb movement, focus and reduced-motion styling. Remove the
  outlined command tiles and nested filled Playback panel; preserve ordinary
  control fills and the Chunks/Options selector's actual selected-state styling.
  Dock and popover share `--audio-dock-inset` so their available width accounts
  for the actual desktop/mobile edge offset; the 900px/32px-root regression no
  longer puts Options controls outside the viewport.
- [x] Keep geometry ownership explicit. AudioControls publishes shell-local
  `--reader-playback-height`, including pending/error feedback, from a layout
  effect before capture and from a ResizeObserver for later size changes.
  The alignment refinement below replaces unconditional clearance with a measured
  `--reader-playback-offset` and explicit beside/stacked layout. DocumentViewer
  measures only its own capsule for `--reader-actions-height`; it does not inspect
  audio internals. Geometry observers/listeners are scoped and cleaned up; no
  polling or book-text observation.
  Long bookmark feedback is height-bounded and scrollable so enlarged text plus
  a tall player cannot push the notice above the viewport.
- [x] Exclude reader tools and bookmark notices from the feature adapter's
  unobscured capture bounds. A regression caught the initial asynchronous-geometry
  version falsely reporting stale capture; synchronously publishing committed
  geometry restores the existing idle viewport/error-retry captures. Matching,
  chunk identity/index, queueing, storage and native audio remain unchanged.
- [x] Browser/unit coverage exercises real DocumentViewer tools with saved audio
  idle/paused/loading/error and without saved audio; bookmark saving, feedback
  clearance, measured heights, normal one-row layout, enlarged wrapping, RTL,
  hit targets, Stop returning to an icon, and unobscured viewport bounds. Switch
  label activation/state/focus, stable AX name/checked state and rate remain
  covered, together with prior pointer/keyboard selection and capture regressions.
- [x] Final validation: **633 passed, 1 existing skip, 83 files**; lint, TypeScript,
  eight-locale validation, production frontend/native build, AppImage packaging
  and bundled GStreamer WAV verification pass. Candidate for that pass (superseded
  by the toolbar refinement candidate below):
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-capsule-controls.AppImage`.
  SHA-256 `c4f856e1701e32266d35a219d02260b905f941888caf625fba29b475ecd03481`.
  Executable copy and frozen runtime/style/locale/test hashes match the tested
  source; `capsule-controls-build.json` records base `438a6989` plus this uncommitted
  pass. Prior named builds remain available; no user app or audio routing touched.
- [ ] Owner consolidated smoke: scroll with saved audio before and after playback,
  use Play/selection/Bookmark/Top, open existing bookmark actions, pause/browse/
  Return, test beginning and canonical continuation, toggle Word Highlight and
  speed, and retry after an unavailable passage. Check the capsule at the previous
  screenshot width, light/dark, large text/RTL, actual keyboard/Space/screen reader
  and selection handles. Owner performs all native/manual listening; automated
  AX snapshots are not screen-reader acceptance.

Removal boundary: the feature-owned shortcut/reader/capture adapters still attach
through the existing optional App/DocumentViewer/audio props. There is less
feature state after removing the session flag. Capsule layout and the geometry
properties are generic reader/player presentation, and the shared switch is a
baseline style with no feature import. They can remain after feature deletion.
Remove the optional shortcut and passage commands through the documented seams;
no user-data, schema, cache or native rollback is introduced.

Next: consolidated owner Linux smoke, then any reported fixes and the remaining
Stage 6/7 device/accessibility, packaged latency, baseline-removal listening and
release-decision gates. Partial durable-span validation and estimated timestamps
remain separate follow-ups. No automatic commit, push, merge or release.

### 2026-10-23 — Aligned tools, clearer disclosure and truthful Save states

Owner authorized all four refinements from the capsule screenshot review. This
is a presentation-only follow-up; matching, source coordinates, chunk boundaries,
resume behavior, queueing, cache records and model/runtime availability rules
stay at their existing boundaries. Source changes remain uncommitted alongside
the preceding capsule pass.

Implementation checklist and acceptance criteria:

- [x] Align the capsule and player on a common bottom edge when their actual widths
  fit at opposite logical edges, with a 0.65em gap. If they collide, stack the
  capsule above the measured player and align their logical end edges. Preserve
  >=44px targets, RTL, large text, safe-area clearance and viewport bounds; no
  fixed device-width assumption decides collision. A live resize can return from
  stacked to beside without reopening the book.
- [x] Keep the coordinator in baseline AudioControls: measure only the generic
  `.reader-floating-actions` box and dock; publish shell-local
  `data-reader-tools-layout` (`beside`/`stacked`), raw
  `--reader-playback-height` and conditional `--reader-playback-offset`.
  DocumentViewer owns capsule CSS and its own height measurement. It never reads
  player internals. A layout effect publishes committed geometry before capture;
  ResizeObserver handles dock/capsule size changes, a shallow child-list observer
  discovers the capsule mounting after scroll, and scoped window/visual-viewport
  resize/scroll listeners handle positioning changes. All disconnect/unsubscribe
  and remove published state on cleanup. No polling or book-subtree observation.
- [x] Compute the candidate opposite-edge position independently of the current
  stacked placement to avoid a latched layout. Use measured outer-edge clearance:
  CSSOM can resolve an `auto` fixed inset to a pixel position, so testing for the
  string `auto` does not identify which edge the CSS anchored. Browser regression
  covers both directions and a wide/narrow/wide transition.
- [x] Keep bookmark feedback above the taller occupied surface even in beside
  mode, since a notice may be wider than the capsule. Bound/scroll long notices
  within the viewport. Stop returns the capsule to its idle edge without stale
  player offset; pending/error-only docks still reserve their measured height.
- [x] Remove the terminal Playback divider; keep the meaningful separator between
  passage commands and settings. No decorative bottom rule under the switch/rate.
- [x] Replace the external hamburger triggers with icon-only horizontal ellipsis
  at all widths (owner follow-up removes the desktop More text). Retain full
  localized More playback options name/title and `aria-expanded` in eight locales.
  Use the ordinary icon-button sizing; remove label-only CSS and translation keys.
  Keep the separate Play action, Chunks list icon, Options sliders, selection
  snapshot, keyboard opening, Escape/focus return and panel contents. Do not add
  ARIA menu semantics to this mixed commands/settings/chunk browser.
- [x] Show saved-ready status as an ordinary tonal check, with the existing saved
  accessible label and disabled/no-duplicate-save behavior. Missing model/runtime
  takes presentation precedence: neutral tonal setup/download action plus warning
  cue and the existing setup action label. A playable import additionally explains
  in the title/name that saved audio can play and setup is for new generation.
  Never combine saved-success and setup-required presentation. Preparing and
  saving/progress retain their earlier precedence and existing callbacks.
- [x] Keep optional passage props/shortcut seams unchanged. No new feature flag,
  service, dependency, persistence, native code or migration. The baseline dock,
  capsule, switch and Save improvements can remain if Play Anywhere is removed.
- [x] Final validation: **647 passed, 1 existing skip, 83 files**, including 47
  isolated Chromium cases; lint, TypeScript, eight locales, production frontend,
  native release, AppImage packaging and bundled GStreamer WAV verification pass.
  New executable candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-toolbar-refinements.AppImage`.
  SHA-256 `d68d86106e5595d28410300e00fca3c385d62abbb909aeb4ba58c7cae636d6e3`.
  `toolbar-refinements-build.json` records base `438a6989`, the uncommitted
  source/test hashes, validation and pending manual gates. Frozen hashes match
  the verified source and copied package; earlier named candidates are preserved.

Owner smoke checklist for the new candidate (all manual/native checks pending):

- [ ] Open the same imported HTML audiobook without a synthesis model/runtime.
  Play saved audio successfully; Save should be neutral with a setup/download
  glyph and warning cue. Its tooltip should explain setup is for generation.
  Check a saved/current setup with a model too: neutral check, truthful label,
  no duplicate save. Verify Save progress/cancel still opens the existing flow.
- [ ] Scroll while playing/paused at the original wide screenshot width. Bookmark
  and Top should share the player's bottom edge when they fit. Resize down to
  320px and enlarge text: stack at the same end edge, without overlap/clipping or
  smaller targets. Widen again and check alignment returns; repeat in RTL/dark.
- [ ] Open More from both the idle header and active player using pointer and
  keyboard. Verify icon-only ellipsis, descriptive tooltip, expanded state, Escape/focus return,
  default Chunks browsing and reachable Options. The Playback section should end
  without a divider; Word Highlight switch and current speed still work.
- [ ] Exercise viewport, selection, beginning, pause/resume, browse/Return and Stop
  on the imported book. Starts remain at canonical chunk beginnings; Resume stays
  at elapsed audio time. Retry after an unavailable passage and check bookmark
  notices/capsule never obscure one another or invalidate a fresh capture.
- [ ] Finish the earlier consolidated smoke plus actual screen-reader/Space,
  selection handles, listening, packaged latency and baseline-removal checks.

Proposed commit message: `fix(reader): align playback tools and clarify menu and save states`

Proposed description: Group reader shortcuts into a compact capsule and align it
with the player when measured widths permit, with a safe stack for narrow or
large-text layouts. Use icon-only ellipsis disclosure with localized accessible names, compact passage
commands, a shared Word Highlight switch and a clean Options surface. Give
saved-ready status a neutral check and prioritize generation setup without
blocking imported-audio playback. Preserve optional feature seams, capture
and selection timing, baseline playback and save/progress callbacks. Update
regression coverage, removal notes and the owner acceptance checklist.

Next: owner consolidated Linux AppImage smoke, then reported fixes and remaining
Stage 6/7 acceptance gates. No automatic commit, push, merge or release.

### 2026-10-23 — Icon-only playback disclosure follow-up

Owner prefers the ellipsis without visible More text at every width, in both
idle header and floating player. Remove both label spans, their special sizing
CSS and eight unused short-label translations. Keep the full More playback
options accessible name/tooltip, expanded state, selection/focus behavior and
existing icon-button dimensions. This supersedes the desktop text-label variant.
The measured dock/capsule layout automatically adapts to the narrower button.

- [x] Update existing header/Options browser checks to require an icon-only,
  named disclosure across narrow/desktop, light/dark and RTL cases.
- [x] 64 focused unit/browser checks pass, together with lint, eight locales,
  TypeScript, frontend/native release build, AppImage packaging and bundled WAV
  verification. Earlier full-suite result is 647 passed/1 existing skip; this
  follow-up reran the affected component/browser suites.
- [x] Latest candidate:
  `src-tauri/target/play-anywhere-acceptance/Papercut-play-anywhere-icon-menu.AppImage`.
  SHA-256 `b9a3483110c1c9f752ef7f2642bd38e259d6c8bad2e772935392ad4a4828f530`.
  `icon-menu-build.json` records frozen source/test hashes and targeted validation.
- [ ] Owner smoke: verify icon-only disclosure from idle header and floating player
  at desktop/narrow widths, descriptive tooltip, keyboard opening/Escape and the
  existing consolidated checklist. Manual native/device/listening remains pending.

### 2026-10-24 — Feature scope and platform acceptance

The owner deferred the first-playback performance investigation until Android,
iOS and other desktop tests show whether a broader change is warranted. This
supersedes the startup-timing, profiling and scanner-packaging follow-ups; earlier
named AppImages/build manifests are historical artifacts, not builds of this tree.

Retained product behavior and removal boundaries:

- [x] Keep the green floppy for a completed current setup with generation ready;
  use the yellow tonal floppy and warning marker when generation setup is missing.
  Imported saved audio remains playable without downloading its generation model.
  Keep icon-only sliders for Playback options in header/player, matching Options.
- [x] Keep browser playback loading until the current `audio.play()` promise
  succeeds. Only then commit the chunk/text/progress and invoke the start callback.
  Stop, seek, document change, Pause and unmount invalidate stale acknowledgements.
  Resume preserves the existing source and exact playback time, including a start
  callback interrupted by Pause. Meaningful player regressions remain.
- [x] Keep the existing immediate, bounded two-chunk desktop lookahead and cache.
  Remove the prefetch comparison checkbox, storage-key access and alternate branch.
- [x] Remove the silent first-playback probe and its UI/tests; startup frame/timer,
  microtask, loading-commit and Long Tasks observers; added phase/read timings,
  read-correlation arguments and optional native timing channel. Keep the original
  bounded TTS diagnostics and existing feature failure/highlight diagnostics.
- [x] Restore native saved-audio read commands to their pre-investigation path.
  Remove the experimental blocking-pool helper and its timing/worker-only tests.
  Command names, source validation, saved-audio payloads and error handling remain.
- [x] Restore existing desktop packaging, AppImage verifier, CI dependencies and
  README launch guidance. Remove scanner-staging changes and their new fixture
  tests; retain the pre-existing required-media verifier. No scanner workaround,
  warmup, audio-transport replacement or new dependency is part of this feature.
- [x] Preserve feature-owned capture/verification/follow adapters and optional
  integration props. Baseline player, native bridge and Save/disclosure styling
  import no Play Anywhere implementation and can remain after feature removal.

Owner smoke checklist (build and manual tests handled by the owner):

- [ ] Test Android, iOS and desktop platforms with a newly built artifact. Record
  platform/OS, artifact/version and whether playback is first use in that process.
  Avoid treating an older timing/profiling AppImage as the current implementation.
- [ ] Open an imported HTML audiobook without its generation model. From a scrolled
  passage and a mouse/touch selection, start at the beginning of the matching saved
  chunk; verify Play from beginning, chunk navigation and natural continuation.
- [ ] Check loading feedback until playback starts, then Pause/Resume at the same
  time. During loading, try Stop, another passage and another book; late results
  must not restore the old passage, highlight or playing state.
- [ ] Check the reader capsule, Return, sliders, compact Chunks/Options, Word
  Highlight switch, speed and green/yellow Save states at narrow/desktop widths,
  large text, light/dark, RTL and with keyboard/screen-reader navigation.
- [ ] Note a repeatable first-start pause or loader warning, if present, and whether
  Stop/replay or another book behaves differently. Platform investigation remains
  deferred; no startup-performance improvement is claimed by this cleanup.

Validation: **654 tests passed, 1 existing skip, 83 files**; ESLint, TypeScript,
eight locale resources and `cargo check --lib --no-default-features` pass. Removed
symbols have no remaining runtime/test references. Packaging/panel files match the
pre-investigation versions, and the staged index and user editor settings are
unchanged. This pass produces no AppImage and leaves existing local artifacts and
user data untouched. Device/listening/accessibility acceptance remains pending.

### 2026-10-24 — Completed-save registry and PDF paragraph verification

Two independent defects were reproduced after the cleanup:

- The save hook inferred completion from `cachedChunks === totalChunks` on the
  last **saving** event. The native save commits its final manifest afterward.
  The queue refreshed the completed registry too early; the later saved event
  left the completion boolean unchanged, so the refresh never ran again. Only
  the native **saved** event or completed command response now confirms readiness.
  Cancel preserves confirmed completion instead of inferring it from chunk count.
- Generation splits sentences within each source paragraph before merging short
  chunks. Verification instead joined the source first, treating a leading
  punctuation glyph in a later paragraph as narrated text. The feature verifier
  now applies the shared sentence grammar to each bounded source fragment before
  comparing a merged chunk. Word changes, independent punctuation, invalid spans,
  source projection and complete coverage still reject. No fuzzy matching, saved
  text/hash/span rewrite, regeneration or native API/schema change is introduced.

The registry fix is baseline save-lifecycle correctness and imports no feature
code. The paragraph-comparison helper stays private to `playAnywhere/verifiedSource.ts`
and disappears with the existing feature verifier. No startup diagnostics or
packaging changes return.

- [x] Reproduce the early completion/finalization failure/cancellation defects
  through the real queue/cache hooks with deferred native results, then verify
  refresh occurs only after finalization, once for event plus command response.
- [x] Reproduce merged-paragraph rejection with a synthetic PDF source; verify
  source points across all merged blocks and retain changed-word/punctuation guards.
- [x] Read-only owner-profile check: all 80 chunks reproduce saved identity; the
  original resolver rejected the book, and the corrected resolver accepts all
  80 canonical chunk starts across 101 narration segments. No private PDF text,
  profile path, manifest or audio is added to fixtures, and app data is untouched.
- [ ] Owner: rebuild; reuse the existing smoke profile and verify its completed
  PDF audiobook appears after reopening the app. Existing audio needs no regeneration.
- [ ] Owner: generate a small new audiobook and confirm it appears immediately
  after completion; test PDF Play/selection on opening and later pages, beginning,
  Pause/Resume and Stop. Repeat relevant platform gates from the checklist above.

Validation: **659 tests passed, 1 existing skip, 84 files**; full ESLint and
TypeScript pass. After preserving the original exact-match path ahead of the
paragraph compatibility check, all **58 affected checks** and scoped ESLint pass.
The synthetic regressions failed before the fixes. No native/package code changed;
no AppImage was built or device/audio test run by the agent. The owner subsequently
rebuilt AppImage and APK artifacts and reports initial testing works as intended;
this is not acceptance of all remaining platform/accessibility gates.

### 2026-10-24 — Known Android first-play permission interruption

Owner testing reports that the first Play on a saved audiobook can open Android's
notification-permission dialog, followed by “The passage changed. Try again” after
either Allow or Block. Pressing Play again works; pre-granted permission avoids
the dialog. This remains an accepted known limitation, not a completed fix.

The pinned upstream native-audio 1.0.5 plugin requests notification permission
inside initialization and resolves without waiting for the user's response.
Passage capture currently precedes initialization; the dialog's lifecycle
interruption can invalidate that capture. This explanation follows the code and
reported behavior; the precise device lifecycle events were not traced here.

- Keep the official upstream Rust/npm dependencies unchanged. The proposed local
  vendor override, plugin patch, test dependencies and CI changes were withdrawn
  at the owner's request; no permission-timing workaround is introduced.
- Keep all hidden/stale/document/options/newest-intent guards. If the first start
  reports a changed passage, dismiss the prompt and press Play again. Enabling
  notifications is not required as a workaround.
- Playback controls may remain visible after denial because Android
  [exempts media-session notifications](https://developer.android.com/develop/ui/compose/notifications/notification-permission#media-sessions).
  This does not grant permission to send ordinary notifications.
- Defer a fix until an acceptable upstream change or supported configuration is
  available. No change to the saved-audio, source-matching or PDF fixes above.
- 2026-10-26: Stage 8 no longer polls passage validity after dispatch, and the
  dialog appears during native initialization, which is after dispatch. A
  connected regression covers a capture invalidated after dispatch on both
  mocks. This limitation is expected to be gone; it stays open until confirmed
  on a device.

### 2026-10-24 — Initial navigation race regression

The review reproduced Next cancelling initial playback while the first WAV read
or native track preparation was pending: transport cancellation invalidated the
passage guard still owned by startup. Gate navigation until first acknowledgement
instead of weakening that guard or adding another ownership API. The connected
coordinator/player/controls test covers desktop and Android paths with delayed
source preparation and delayed Play acknowledgement, disabled navigation attempts,
then successful Next and chunk-list takeover after startup. The native mock now
honors the same pre-command predicate as the maintained playback adapter.
Removal also drops this connected test and its coordinator/control imports from
`useTtsPlayer.test.tsx`; ordinary player tests and the UI gate remain independent.

Validation: full frontend suite **663 passed, one existing skip**; the final
player/reader-control checks **90 passed**, with TypeScript and ESLint checks.
No native/plugin, dependency or packaging changes; no device smoke performed.

Owner smoke checklist (pending):

- [ ] Cold Play from here/selection/beginning: Previous/Next and chunk-list rows
  are disabled until playback starts; Options opens and Stop remains available.
- [ ] After start, Next and a chunk-list jump work; subsequent loading still
  permits rapid navigation. Stop during initial loading prevents late playback.
- [ ] Switching/closing the book or invalidating the passage during initial
  loading cannot start stale audio. The known Android permission retry remains.

### 2026-10-24 — PR review readiness and fixture corrections

The controls and coordinator now share `audioControlsAudiobookState.complete`.
It accepts the active save only when its document/settings identity matches the
open audiobook, and only after confirmed finalization. A stale independent cache
check cannot reject Play immediately after Save. Source, options, newest-request
and player guards remain unchanged; no readiness logic moves into the feature
resolver. Removing the feature retains the manager's normal readiness derivation.

Four manager/coordinator regressions cover beginning and passage starts after a
matching save, plus rejection of completed saves for another document or voice.
Both successful-start cases reproduce `not-ready` against the pre-fix manager.
Remove their feature-specific action assertions with the optional coordinator;
the existing queue/cache finalization tests remain baseline coverage.

Reader-control CSS imports resolve from the checkout root. The PDF browser
fixture writes its bundle and case code into external JavaScript files instead
of interpolating executable code into HTML. A literal closing-script payload
round-trips as data; no production sanitizer, dependency or CodeQL exclusion is
introduced. These fixture changes disappear with the feature's browser tests.

Validation: **668 passed, one existing skip, 85 files** with `CI=true` and two
CPUs; TypeScript, ESLint and the production frontend/Pagefind build pass.
Six header cases also pass from a relocated checkout. The closing-script case
fails against the original helper and passes against the external-script fixture.

The reported CI PDF timeout remains unconfirmed: the original eight PDF cases
pass in isolation, and the updated nine pass both focused and in the two-CPU
suite. The shared 15-second fixture deadline is unchanged. No test is disabled,
and the D-Bus stderr is not established as a cause. Remote CI/CodeQL confirmation
and platform smoke remain pending; this is not a claimed timeout repair.

Owner smoke checklist (pending):

- [ ] Save a new audiobook while its document stays open, then immediately use
  Play, Play from selection and Play from beginning without reopening it.
- [ ] An incomplete save or a completed save for another document/setup must not
  enable playback. Stop/close/switch during preparation still rejects late work.

### 2026-10-24 — deletion readiness and CI timeout follow-up

Successful saved-audiobook deletion now calls the save queue's
`invalidateCompleted(record.id)` before resetting the open-document cache.
The queue clears only the captured finalized save with that settings identity;
object identity also protects a newer save with identical settings. Starting
another save invalidates the old completion token. Cache reset rejects any late
response from the forgotten save command. Cancelled/failed deletion and unrelated
or in-progress saves keep their state. This is baseline save/delete ownership,
retained if Play Anywhere is removed; passage and player guards are unchanged.

The manager regressions now exercise the real queue, cache and saved-action hooks
with mocked native I/O. Added cases cover deletion success/failure/cancellation
and a replacement save completing before an older deletion returns. Queue cases
also cover unrelated identities, a newer save with identical settings, an active
save, and a late final command response after invalidation.

The latest CI failure has a confirmed timeout-ordering defect: the script-data
regression used Vitest's default 5 seconds, shorter than the browser driver's
15-second deadline. Its still-running browser held the shared profile lock and
subsequent launches failed. That test now uses the suite's existing 30-second
budget, leaving the driver time to clean up. The driver deadline is unchanged;
no browser test is skipped. The earlier independent 15-second PDF timeout remains
unreproduced. CodeQL alerts 7/8 and their fixture construction are unchanged.

Validation: **677 passed, one existing skip, 85 files** with `CI=true` and two
CPUs, including all nine PDF browser cases. The 17 connected manager/queue checks
pass; the deletion-success regression fails without the manager invalidation call.
ESLint, TypeScript, eight-locale validation and the production frontend/Pagefind
build pass. Remote CI and platform acceptance remain pending.

Owner smoke checklist (pending):

- [ ] Save, then delete the matching audiobook while its document stays open:
  Play becomes unavailable without reopening the document.
- [ ] Cancel deletion, or observe a failed deletion: the existing saved audio
  remains playable. Deleting another saved setup does not clear its completion.

### 2026-10-26 — Cancellation architecture review (Stage 8 spec)

This was a read-only merge review; no source changed. The owner reports
desktop/Android normal-playback smoke and playback of `main`-generated imported
audiobooks working. iOS waits for a post-merge TestFlight build.

- **Probe.** A temporary reader test (deleted afterwards) used the real playback
  scope with delayed preparation:
  - it called `playback.cancel()`, which is what the manager's voice/model
    handlers call, while `pending` was true;
  - then it resolved the target.
  - Result: `{ feedback: 'stale', pending: false }`, and nothing was dispatched.
    The UI shows `tts.playAnywhere.stale` (“The passage changed. Try again.”)
    after a deliberate settings change.
- **Code reading.**
  - One mechanism answered three questions (intent, validity and lifetime). The
    mechanism was `isCurrent`/guard closures that every layer polled and mapped
    to `stale`.
  - Cancellation lived in five places:
    - the reader's intent counter;
    - the playback scope's intent counter and AbortController;
    - the preparation controllers;
    - the viewer capture epochs;
    - the player's job, navigation and attempt ids, plus
      `requestGuardRef`/`navigationGuardRef`/`targetStartedRef`.
  - **Opt-in wrappers.** App wrapped 6 transport handlers with
    `readerPlayback.cancel()`. The manager wrapped the same 5 transport
    handlers plus 4 settings handlers with `cancelTargetPlayback()`. Any new
    control that forgot a wrapper could let a stale target start late.
  - **Guards polled after dispatch.** `isCurrentJob()` bumped the job and called
    `finishPlayback()` whenever an external guard turned false. As a result:
    - Next during startup idled playback. The 2026-10-24 race was contained by
      the UI gate, not fixed.
    - The Android permission dialog cancelled a start that had already been
      dispatched (the 2026-10-24 limitation).
    - `prepareDocumentOpen` killed a still-starting target but not a playing
      one; on `main` it never stopped audio.
  - **Dead bookkeeping.** The `targetStartedRef` that pause/resume preserved
    did nothing after App's reader cancel.
- **Decision.** Adopt §3 “Cancellation model” and implement it as Stage 8 before
  merge. Comment-only and wrapper-helper options were rejected (see §8).

### Stage 8 — Cancellation consolidation (merge blocker)

Order: tests first, additive player change, then switch consumers, then delete.
Every step keeps the full suite green except the tests intentionally added red in
step 1.

- [x] **1. Characterization tests.** Add the red cases next to the existing
  connected coordinator/player/controls tests:
  - (a) A settings change, delete, Pause or Stop during preparation leaves no
    feedback and nothing plays. Red today.
  - (b) A newer target supersedes an older one; stale capture before dispatch
    still rejects; dispose before dispatch never plays. These pin today's
    behavior.
  - (c) A transport command issued only through the player cancels a pending
    target, without any wrapper. Red today.
  - (d) Skip/jump during a dispatched startup navigates on the desktop and
    native mocks, instead of idling. Red today.
  - (e) Capture validity flipping after dispatch (the native mock delays init)
    does not cancel startup. Red today.
  - (f) Ticket semantics: `started`; `superseded` by Pause or a newer target;
    `failed` on a Play rejection.
- [x] **2. Player epoch and ticket (additive).**
  - Add the command-epoch controller, `commandSignal()` and the start ticket
    (`Promise<PlaybackStart>`) alongside the existing guards.
  - Resolve the ticket at the existing desktop and native acknowledgement
    points, and in `fail()`.
  - Existing tests stay green.
- [x] **3. Target scope.**
  - `createPlayAnywherePlayback` forwards the epoch into its run controller,
    returns `cancelled` for aborts (`disposed` when it is no longer alive), and
    returns the ticket in `dispatched`.
  - It stops passing guard/`onStarted` to the player.
  - (a), (c) and (f) go green.
- [x] **4. Reader.**
  - Consume the ticket and stop showing feedback for `cancelled`.
  - Clear feedback through the once-listener on the epoch. As built, the
    selection snapshot is left to the input epoch; see §3 rule 6.
  - Remove `cancel()` and the `cancelTargetPlayback` option.
- [x] **5. Remove wrappers.**
  - In App, delete the six `readerPlayback.cancel()` handler overrides.
  - In the manager, delete:
    - the `cancelTargetPlayback()` calls in pause/resume/jump/skip and the four
      settings handlers;
    - the `cancelTargetPlayback` export;
    - `stopDocumentPlayback` (callers use `stopTts`);
    - the cancel in `prepareDocumentOpen`, since scope disposal covers it.
- [x] **6. Remove player guards.**
  - Delete `requestGuardRef`, `navigationGuardRef`, `targetStartedRef`, the
    guard/`onStarted` parameters and the pending-start carryover.
  - Make `isCurrentJob` pure.
  - (d) and (e) go green.
  - `rg 'cancelTargetPlayback|requestGuardRef|navigationGuardRef|targetStartedRef' src`
    returns nothing.
- [x] **7. Docs.**
  - Rewrite the §3 contract rows (player initial start, player active target,
    target playback action) and the coordinator paragraph to match.
  - Update the removal-map rows for App, the manager and `useTtsPlayer`.
  - Mark the Android limitation entry as superseded or confirmed after the
    device check.
  - Drop the “not implemented” status line from §3 “Cancellation model”.
- [ ] **8. Verify.** Automated checks done; device smoke and browser tests
  pending (see the 2026-10-26 implementation entry).
  - Run the full frontend suite with `CI=true`, plus TypeScript, ESLint and the
    build.
  - Desktop and Android smoke, all exercised during preparation and during
    startup: play from here, play from selection, play from beginning,
    pause/resume, skip, chunk jump, Stop, voice/model change, delete, document
    switch/close, and the Android first-play permission dialog.
  - iOS remains the post-merge TestFlight check.

Acceptance:

- A new transport or settings control needs no play-anywhere code to cancel
  pending targets.
- `cancelled` never produces user feedback.
- No guard is evaluated after dispatch.
- The diff to `useTtsPlayer.ts`, `useAudiobookManager.ts` and `App.tsx` removes
  more lines than it adds.
- Removal of the feature still only needs the seams in the removal map: the
  epoch and ticket are general player API.

### 2026-10-26 — Stage 8 implementation

Steps 1–7 are implemented. The step order was followed: red tests, an additive
player epoch/ticket, then the scope, the reader, wrapper removal, and finally
guard deletion.

- **Removed APIs:**
  - `requestGuardRef`, `navigationGuardRef` and `targetStartedRef`;
  - the `guard`/`onStarted` parameters of `speak`, `jumpToChunk`,
    `startPlaybackAt` and `playFromChunk`;
  - the pending-start carryover in pause/resume;
  - `reader.cancel()`, `cancelTargetPlayback` and `stopDocumentPlayback`;
  - every App and manager cancellation wrapper.
- **Behavior changes:**
  - A settings change, delete, preview, Pause or Stop during preparation no
    longer shows “The passage changed”; nothing plays.
  - A starting target continues across a document switch, as ordinary playback
    does. Before, the outcome depended on timing.
  - The Android permission dialog after dispatch no longer cancels startup.
    This narrows the 2026-10-24 entry's “keep all guards” to “guards up to
    dispatch”.
  - A skip that reaches startup navigates instead of idling.
  - Pausing with the passage menu open keeps the selection snapshot.
- **Review fix.** The first draft only aborted the epoch on unmount. Under
  StrictMode (`src/main.tsx`), effect replay would then have left an aborted
  epoch and cancelled every dev-build target. Unmount now replaces the epoch,
  and a regression test fails against the first draft.

- **New tests:**
  - player: an epoch per public command but not auto-advance; StrictMode epoch
    replay; `started` on Linux/Android; `superseded` by
    pause/stop/skip/jump/target/unmount; `failed` on Linux/Android;
  - connected: a command cancels preparation without a wrapper; validity is
    not re-checked after dispatch; skip during startup keeps playing (all on
    both mocks);
  - scope: command/dispose/newer → `cancelled`; a validity check before
    dispatch returns the ticket;
  - reader: command/settings during preparation → no feedback; feedback
    cleared by the next command; a superseded ticket clears `pending` without
    restoring follow.
- **Removed tests:** the tests that asserted post-dispatch passage rejection
  were removed on purpose. This covers the player's “stale during native
  preparation” and “late chunk after stale” cases, and the guard and
  `onStarted` assertions. The new tests assert the opposite. The Pause/Resume
  test now expects `superseded`.
- **Size:** `useTtsPlayer.ts`, `useAudiobookManager.ts` and `App.tsx` remove
  more lines than they add (110 added, 116 removed). The playback scope grew by
  21 lines to carry the explicit `cancelled` outcome and the epoch forwarding.
- **Validation:**
  - `CI=true` non-browser suite: **613 passed, one existing skip**;
  - TypeScript (`tsc -b`), `eslint .`, i18n validation and `vite build` pass;
  - `rg 'cancelTargetPlayback|requestGuardRef|navigationGuardRef|targetStartedRef|stopDocumentPlayback' src`
    returns nothing.
- **Not run here:** the agent sandbox cannot launch Chromium. All
  `*.browser.test.ts` files and `LibraryTransferDialog.layout.test.tsx` fail
  there with connection or launch errors, and they failed the same way before
  any change. `readerControls.browser.test.ts` was updated to the ticket API
  and must be run locally.

Owner checklist (pending):

- [ ] Browser tests locally: `npx vitest run src/tts/playAnywhere` (includes the
  `*.browser.test.ts` files).
- [x] Desktop and Android, once during preparation and once during startup
  (owner, 2026-10-26):
  - Play from here, Play from selection, Play from beginning;
  - Pause/Resume, skip, chunk jump, Stop;
  - voice/model change, delete, document switch/close.
  - Expect no “passage changed” notice after your own action.
- [ ] Android first-play notification-permission dialog: startup continues
  after Allow or Block.
- [ ] Dev build (StrictMode): Play from here starts on first use.

## 7. Standards and design rationale

This plan applies separation of concerns, explicit capability contracts,
backward-compatible defaults, deterministic state transitions, validation at
boundaries, bounded resource ownership, observable failure reasons and regression
coverage. These are engineering design choices, not a claim of certification.

- [WCAG 2.2](https://www.w3.org/TR/WCAG22/): check keyboard operation (2.1.1),
  no keyboard trap (2.1.2), focus order/visibility/not obscured (2.4.3, 2.4.7,
  2.4.11), target size minimum (2.5.8), contrast (1.4.3, 1.4.11), reflow (1.4.10)
  for applicable reflowable UI, names/roles/values (4.1.2), and status messages
  (4.1.3). Use existing touch-sized controls, aiming for 44 CSS px where practical;
  WCAG 2.2 AA's 2.5.8 minimum is 24 CSS px with its specified exceptions, not 44.
  Avoid screen-reader announcements on every chunk/timeupdate. These selected
  checks do not establish whole-app WCAG conformance.
- [EPUB 3.3 Media Overlays](https://www.w3.org/TR/epub-33/#sec-media-overlays)
  and [Readium Guided Navigation](https://readium.org/guided-navigation/) pair
  text locations with audio intervals. Reuse that principle with existing
  Papercut source spans/timing; this feature adds no EPUB overlay compatibility.
- [CSSOM View caret position API](https://drafts.csswg.org/cssom-view/#dom-document-caretpositionfrompoint)
  and [MDN compatibility guidance](https://developer.mozilla.org/en-US/docs/Web/API/Document/caretPositionFromPoint)
  inform hit-testing. The linked CSSOM View editor's draft is work in progress,
  not a finalized conformance requirement. Feature-detect and verify actual
  shipping WebViews rather than assuming the method exists everywhere.
- [Readwise Reader TTS](https://docs.readwise.io/reader/docs/faqs/text-to-speech)
  documents viewport starts, browse suspension and return/jump actions.
  [Speechify](https://speechify.com/blog/listening-features/) and
  [Microsoft Immersive Reader](https://support.microsoft.com/en-us/accessibility/word/use-immersive-reader-in-word)
  document targeting text to start listening. These are interaction precedents,
  not evidence about competitors' internal implementation. Papercut chooses the
  first visible narrated passage and a saved chunk boundary explicitly.

## 8. Rejected alternatives

| Alternative | Why not selected |
| --- | --- |
| Scroll percentage -> audio percentage/chunk | Fails on images, reflow, uneven chunks and PDF layouts. |
| Build/measure a DOM Range for every chunk on each action | Unnecessary book-sized layout work and poor fit for PDF virtualization. |
| Start at zero, then call jump | Can play the opening fragment and move the reader before the intended seek. |
| Slice chunks before calling the player | Changes canonical indices/signature and breaks saved-audio identity. |
| Approximate word timing or forced alignment | Unneeded for the accepted chunk-start behavior. |
| New player, global event bus, feature-flag framework or persistent settings | Duplicates ownership and makes debugging/removal harder. |
| Keep the cancellation wrappers and add comments naming each other (2026-10-26) | Documents the hazard but keeps opt-in cancellation and the false `stale` feedback. |
| One manager helper that wraps every transport/settings handler | Still opt-in at a single list, and still polls the guard after dispatch. Supersession belongs where commands are ordered, in the player. |
| Keep polled guards but carry a cancel reason | Fixes the feedback, but keeps the side-effecting `isCurrentJob`, the post-dispatch Android cancellation, and the timing-dependent document-switch result. |
| `AbortSignal.any` / `signal.reason` | Not available on the iOS 15.0 minimum. A once-listener forward and a typed result do the same job. |
