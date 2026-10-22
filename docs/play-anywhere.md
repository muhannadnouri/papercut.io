# Play anywhere: integration plan

Status: resolver foundation implemented in isolation; reader/player integration
has not started. Stage 0 code audit and Stage 1 pure resolver checks are recorded
below; baseline listening/device gates remain open.
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
| User browses during playing or paused playback | Suspend automatic following. Expose **Play from here** and **Return to audio** in the playback controls; retain normal transport controls. |
| Play from here while paused | Seek to the chosen chunk beginning and play; a seek that remains paused does not satisfy this action. |
| Return to audio | Reveal the current audio passage and enable following. Preserve playing/paused state and current audio time. |
| Play from beginning | Explicit audio-menu action; start canonical chunk zero. Also provides an accessible fallback when location mapping is unavailable. |
| Explicit selection action | **Play from here** in the reader selection/context action targets the selection's start in document order, then plays its containing chunk. |
| Stop | Retain current stop semantics and end the browse/follow session. A later Play from here captures the then-current viewport. |
| Missing/incomplete saved audio | Preserve current save/setup affordances. This feature does not generate missing audio. |

Use the existing button, menu, icon, focus, and responsive styling patterns.
The primary play button needs an accessible name and tooltip; any extra viewport
action shown while browsing should have a visible label. “Chunk” is unnecessary
in primary UI copy. Supporting description: “Starts at the beginning of the audio
segment containing this passage.” Keep selection, hyperlinks, and current
fullscreen toolbar gestures working normally. No new global shortcut is needed.

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
| Feature-owned code under `src/tts/playAnywhere/` | Pure lookup is implemented; capture-to-chunk orchestration, format mapping and identity checks follow in later stages. Keep only files that earn their existence. | Native audio session, new persistent settings, parsing/import pipelines. |
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

Names below are proposed contracts, not claims that exports already exist.

| Contract | Input/output and lifetime |
| --- | --- |
| Optional reader target API | Register/unregister through the existing `ViewerProps` callback pattern. Capture `viewport` or `selection` at action time; return an ephemeral snapshot or typed unavailable reason. No audio side effects. |
| Target snapshot | Carries current document/view revision and format-specific point. Capture live DOM node/offset synchronously if conversion needs deferred indexing; validate node/root membership and revision before converting. Never serialize DOM references. |
| Normalized source point | HTML: readable segment index + normalized UTF-16 offset. PDF: page index + original text-item/block order + source offset. Do not conflate the two coordinate systems. |
| Resolver result | `resolved` with canonical chunk index/id, or `unavailable` with `not-ready`, `unmapped`, `ambiguous`, `invalid-source` or `invalid-point`. Capture/coordinator additionally own `no-visible-text` and `stale`; the pure resolver does not inspect DOM or session identity. Unexpected errors remain errors rather than successful fallbacks. |
| Player initial start | Add an optional start index, defaulting to zero for existing callers. It refers to the same canonical nonempty chunk sequence used by playback/native signatures. Pass the complete sequence, never a sliced tail. |
| Player active target | Reuse existing navigation serialization and newest-intent checks. Explicitly support seek-then-play for this action without changing other callers' paused-state behavior. |
| Follow policy | Optional scroll/follow input with the current behavior as the default when omitted. Gate all automatic HTML/PDF reveals and cancel pending reveals. Keep highlight updates independent. |

At the coordinator boundary bind each request to selected document, current view
revision, selected audiobook identity/options, chunk sequence revision, and a
monotonic action id. Reuse existing identities/references where sufficient; do
not hash the entire book per click. Recheck after each await and immediately
before the player side effect. Stop, model/voice/audiobook change, document close,
and unmount invalidate requests. A late completion cannot restart stopped audio,
play a different book, update a replacement view, or override a newer target.
Existing player generation guards remain authoritative after dispatch.

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
Stage 2 must retain a transient line-offset -> original block/order/offset map
through the same line-building code, including inserted spaces, removed newlines
and trimmed line ends. Share that conversion instead of independently rebuilding
OCR lines in this feature. No stored OCR schema change is needed; do not claim
OCR capture/highlighting support until the actual overlay path is verified.

### Playback paths and compatibility

- Desktop: enter existing navigation/loading at the chosen index, play its WAV
  from `currentTime = 0`, then use existing bounded prefetch/cache pruning.
- Mobile initial start: prepare/reuse the existing seekable track, set source and
  rate, await seek to the validated chunk's actual `startSec`, then play. Guard
  every async transition. Do not audibly play zero and jump afterward.
- Mobile active session: serialize seek and play with existing lifecycle helpers;
  rapid target changes must not play a superseded location. Preserve source/session
  reuse and background/lock-screen ownership. Do not add a parallel command queue.
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
| `src/tts/playAnywhere/` | `sourceChunkResolver.ts` and its focused test exist; adapters/coordinator follow later. | Delete feature-owned implementation/tests after disconnecting callers. Currently tests are the resolver's only callers. |
| `src/viewers/types.ts`, `HtmlViewer.tsx`, `PdfViewer.tsx` | Optional target capability registration; PDF follow input. Helpers stay out of large viewer components. | Remove optional capability props/registrations/type imports and feature-only helpers; retain normal viewer behavior. |
| `src/components/DocumentViewer/DocumentViewer.tsx`, `src/App.tsx` | Wire capability refs, actions and session-local follow state; clean up on view replacement. | Remove feature wiring/state; ordinary controls/bookmarks/Find still work. |
| `src/tts/hooks/useAudiobookManager.ts`, `useAudiobookDocument.ts` | Supply prepared chunks even before playback; select correct audio identity; expose actions. | Disconnect coordinator/actions; retain original preparation and legacy highlight recovery. |
| `src/tts/hooks/useTtsPlayer.ts` | Backward-compatible start index and narrowly scoped seek-and-play support. | Omit start index to recover zero-start behavior; remove unused additions after checking all callers. Retain original navigation guards. |
| `src/tts/alignment/domTextSegments.ts` and imported-span helpers, only if needed | Reuse/export the smallest shared inverse mapping or validated span recovery. | Remove feature-only exports after a caller search. Preserve helpers still used by highlighting. |
| `src/pdf/ocr/pdfOcrTextLayer.ts`, if needed in Stage 2 | Expose transient source-block offsets alongside existing selectable OCR lines. | Remove feature-only inverse mapping consumers/exports; retain line rendering and persisted OCR data. |
| `src/tts/hooks/useTtsHighlight.ts`, PDF highlight/reveal path | Optional following control and pending-scroll cancellation. | Omit the optional policy to retain baseline automatic following; keep unrelated highlight fixes. |
| `src/tts/components/AudioControls.tsx` and existing styles | Optional viewport/selection/beginning/return actions and accessible states. | Remove feature actions/styles and restore baseline idle play wiring. |
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
There is no existing reader selection/context action API to extend; Stage 5 must
add a small explicit accessible action rather than assume a ready-made menu.

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

- [ ] Implement optional HTML and PDF capture capability registration/cleanup.
- [ ] Resolve first visible narrated text beneath occluding controls and preserve
  source order across columns. Support nested scroll containers, fullscreen,
  zoom, rotation, images, reflow and enlarged text.
- [ ] Reverse-map PDF text items and recognized-text source runs; handle missing
  virtualized layers as not-ready. Validate fallback capability detection for
  `caretPositionFromPoint` / existing WebKit caret fallback.
- [ ] Test DOM replacement, Find marks, reader close and stale captured nodes.

Acceptance: rendered-browser fixture tests assert actual geometry/selection
mapping rather than relying on jsdom layout. Supported visible passages resolve
to expected chunks after reflow/Find/zoom; unsupported pages report unavailable.
Absent capability props leave ordinary reading behavior unchanged.

### Stage 3 — Initial start and active navigation

- [ ] Add backward-compatible starting index to the existing player entry point.
- [ ] Desktop loads/plays the target chunk first and keeps current prefetch bounds.
- [ ] Mobile awaits source/rate/target seek before play; explicit active
  seek-and-play works while paused and after foreground synchronization.
- [ ] Coordinate capture/resolve/dispatch with document/audio identity and newest
  request guards. Preserve full canonical sequence and native signature.
- [ ] Test rapid A->B->C targets, stop/close/model change during each await, late
  native replies, missing/corrupt chunk, preparation and seek/play rejection.

Acceptance: first audible content is the requested chunk's beginning; no chunk
zero blip. Paused Play from here actually plays, Resume retains exact position,
newest intent wins, and failed resolution leaves current playback intact.
Existing callers without a start index preserve their behavior.

### Stage 4 — Controls and follow/browse behavior

- [ ] Wire stopped Play from here, explicit Play from beginning, and browse-mode
  Play from here / Return to audio through existing controls.
- [ ] Separate following from playing/paused state; gate HTML delayed scroll,
  PDF page changes and every automatic highlight reveal.
- [ ] Handle user inputs, explicit reader jumps, momentum, programmatic scroll,
  pending reveal cancellation, reduced motion and teardown.
- [ ] Add all eight locale keys and accessible loading/unavailable messages.

Acceptance: scrolling ahead never snaps back at the next chunk; highlighting
continues. Return to audio reveals without seeking or resuming paused playback.
Successful Play from here restores following, with later user browsing taking
precedence. Controls remain usable in mobile/fullscreen/RTL/enlarged-text layouts.

### Stage 5 — Selection entry point

- [ ] Add the explicit selection/context action without replacing normal
  selection, link activation or platform accessibility gestures.
- [ ] Capture selection before focus loss; validate endpoints/current document
  and map the document-order start through the same resolver/coordinator.
- [ ] Cover keyboard, touch long-press, backward/multi-node selections, skipped
  references, cross-reader selection and selection invalidation.

Acceptance: selecting anywhere within a chunk starts that chunk from zero;
viewport actions remain independent of incidental selection. A keyboard user
can invoke the action and recover focus; unavailable selections explain why.
If platform menu limitations prevent this, document the limitation and resolve
scope explicitly rather than shipping an inaccessible gesture substitute.

### Stage 6 — Regression, accessibility and performance gates

- [ ] Add focused resolver, adapter, coordinator and player lifecycle regressions
  for new behavior, including baseline behavior with optional capabilities absent.
- [ ] Run relevant focused tests, then `npm test`, `npm run build:typecheck`,
  `npm run check:i18n`, `npm run lint`, and `npm run build` before integration.
  Run native tests/checks only if native code changes; retain existing CI matrix.
- [ ] Complete the device/format matrix below and performance budgets above.
- [ ] Verify unchanged saved source/audio/chunk identity fixtures and existing
  bundle import/export, transfer, Find, bookmarks, model settings and lock-screen
  controls. No private corpus excerpts enter tracked fixtures or diagnostics.
- [ ] Review dependency direction, cleanup, error boundaries and removal scope;
  record any added/changed integration surfaces here.

Acceptance: no failing required check, no unexplained regression, no unmeasured
performance claim. Relevant WCAG checks pass with keyboard and screen reader;
device listening confirms target content and ordering. Build/CI success alone
does not establish audible, accessibility or performance acceptance.

### Stage 7 — Removal rehearsal and release decision

- [ ] In a disposable patch/worktree, disconnect the optional feature coordinator,
  restore the original idle-play action and omit follow-policy/start-index inputs.
  Do not reset/delete user work to rehearse removal.
- [ ] Verify baseline playback/reading still works without any feature-owned
  runtime module; check compile, focused baseline tests and a playback smoke.
- [ ] Confirm removal needs no user-data deletion, schema downgrade, cache
  regeneration or changes to import/search/bookmarks/transfer.
- [ ] Restore the intended feature patch and rerun checks affected by restoration.
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
| Repeated lifecycle + removal rehearsal | Stop/close/switch/cancel races; released resources; baseline with feature disconnected. | Pending |

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
