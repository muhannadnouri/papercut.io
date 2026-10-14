# Search v2 consolidated smoke pass

Status: **partial user-reported installed-app smoke; further smoke deferred**.

Reported on 2026-10-14 after the user supplied [successful CI run 38500026388](https://github.com/muhannadnouri/papercut.io/actions/runs/38500026388) (application source `15b3710`):

| Environment | Reported coverage | Unconfirmed details |
| --- | --- | --- |
| CachyOS Linux desktop | Some smoke tests; behavior appeared to work as intended. Small FP32 selected. | Specific cases, installed SHA/architecture and fresh-versus-existing library not established. |
| Android 17 | Downloaded CI artifact; Small FP32 model download, semantic enablement, explicit library indexing and a successful conceptual search. | Device model, physical/emulated hardware, architecture, exact installed SHA and fresh-versus-existing library not established. |
| iOS | Not conducted. | Installed-app coverage remains open. |

This is the user's report, not independently observed automation. Do not mark whole checklist rounds passed: restart/upgrade, cancellation/error recovery, exact reader/Find, RTL/accessibility, large-library responsiveness and other model formats were not confirmed. Windows/macOS installed-app smoke is also unconfirmed. The earlier normal/cache smoke reported on 2026-10-09 remains historical. No checkbox below is marked passed by automated tests or these partial reports.

The new Keyword/Conceptual result tabs, grouped tonal controls and bounded lists postdate that artifact and need fresh current-source CI and targeted installed-app checks. Further smoke is explicitly deferred for this implementation pass. Merging is intended to release immediately, so outstanding iOS/platform/distribution acceptance remains a release gate.

Use this as **one rebuilt-app pass**, reusing a disposable profile and sources for overlapping cases below. Keep private questions, sources, screenshots and individual logs outside tracked files. The [current acceptance follow-up](search-v2-branch-review.md#result-view-follow-up-and-reported-smoke--2026-10-14) separates verified automation, reported smoke and remaining release gates.

## Result-view checks — deferred

- With semantic search off, confirm ordinary bounded Keyword results and no result-view selector or conceptual fallback notice. Turning it on must not download or index implicitly.
- Confirm the tonal Keyword/Conceptual selector and Keyword-only matching options share a compact row directly below the search bar, with helper text before the shared filter. Options stack on narrow screens; only the compact control row sticks while input/helper/filter scroll. Check centered options, helper and examples with semantic search off and on. Confirm compact desktop segments, 44px targets with touch (including mixed mouse/touch devices), icons left of visible labels, keyboard focus and correctly directed RTL/zoom labels. Keyword example chips appear only for an empty Keyword query and fill it without submitting; Conceptual hides them. Switching keeps an open filter and the shared query intact.
- With a large library, confirm Keyword/Conceptual tabs stay reachable while scrolling, start with ten cards and reveal batches through Show More. Counts must describe returned documents in the shared scope; loading/failures must not claim zero matches or an exhaustive library total.
- Switch using touch and keyboard; retain the chosen view across subsequent searches, expanded evidence and each view's scroll place when switching. New queries/scopes reset the list window. A late result must never switch the view. All Words/Broader controls are associated only with Keyword.
- Select Conceptual while loading and while missing/partial/failed; fallback should remain readable and direct the user to Keyword, whose results remain independently usable. Confirm active-panel focus/live announcements, narrow/zoom/RTL layouts and sticky controls in the actual WebView.
- Open results after More; reader targets must still use full original text/page/section metadata. Confirm scroll restoration when switching between lists of different lengths. These DOM regressions do not verify WebView layout or device performance.

## Added review-fix checks — deferred

- Import generic HTML/EPUB with body text before/after an inline footnote; confirm the note remains literal-searchable and reader links/locations survive, while conceptual primary evidence prefers the body spans. Reopen a schema-12 disposable library: affected vectors require explicit update, interrupted repair resumes, and missing/mismatched sources preserve their text/classification.
- Start an explicitly permitted model download, close Settings and interrupt the connection. Confirm **Model Download Failed / Review** appears after native work ends; reopen to see the model diagnostic and cleanup/retry controls. Status retry or Review must not discard the diagnostic; a new permitted download clears it. Success does not enable the toggle or start indexing.
- Submit more than 128 combined broad/quoted terms or 4,096 Unicode characters: confirm the inline limit error and no starter/native/conceptual work. Submit a valid retry; changing/clearing a query must stop old starter term lookups without publishing stale results.
- If an indexing failure occurs, confirm the diagnostic names the document ID and section number when known, without adding document title/passage/query text. Retry explicitly: completed checkpoints should be reused. Stop Indexing must still report cancellation rather than a contextualized failure.

Automated boundary regressions cover these contracts; the interactive cases above
are not marked passed.

Review items 5–7 now have real React/DOM interaction and parser/Find integration
regressions in the normal `npm test` suite. These use jsdom visibility fixtures
and assert source match offsets; they do not paint PDF canvases or validate actual
browser layout, keyboard default traversal, touch targets or screen-reader output.
The reported subset above does not accept these interactive review-fix checks; keep further checks deferred until the user resumes this pass.

## Build and Run Order

Normal native shipping builds include provider capability; semantic use still
starts off and weights are not bundled. From the repository root:

```sh
npm run desktop
```

On Android use `npm run android:apk:native-tts`; on macOS for iOS use
`npm run ios:ipa:native-tts` with the appropriate signing/device setup. Launch the
built package as well as any diagnostic binary. No separate semantic npm script
or BGE scorer build is required. `desktop:static` remains a lexical-only
alternative, not the build for the semantic-on checks.

| Round | What to cover |
| --- | --- |
| 1. Fresh Profile / Semantic Off | Isolate data first; import inspectable HTML/EPUB/text PDF. Check lexical modes, quotes, filters, typo/Arabic forms, unified result counts and source opening. Opening Settings/searching starts no model or index work. |
| 2. Explicit Setup | Exercise model permission, download/verify, cancellation, reopen, recovery and header activity. Start with Small FP32 for the baseline; record other chosen models/formats separately. Downloading does not enable use. |
| 3. Semantic On / Index Lifecycle | Enable the cached model, explicitly build/update, cancel/resume and restart. Check ready/partial/empty states and foreground reader/audio interaction. Use the preparation controls cases below. |
| 4. Conceptual Results / Source Evidence | Check separate results, scopes, required quotes, latest-request recovery, missing/partial fallback and disabling during work. Expand and open primary/nearby text in its own original location; judge actual source support, not just similarity. |
| 5. Upgrade / Stored Work | Complete the main-content, natural-boundary and PDF backmatter follow-ups below in the same pass. Update affected old PDF coverage/rebuild incompatible windows; reimport role-losing old EPUBs where needed. Test model switching and guarded removals only in disposable data. |
| 6. Package / Accessibility / Responsiveness | Check package launch, keyboard/focus, narrow/zoom/dark/RTL/reduced-motion views and short reader/audio/audiobook flows. Record slow cleanup, stalls or crashes. Mark unavailable physical/minimum-OS or larger-model checks not run. |

A conceptual nearest match can be incomplete or irrelevant when the library lacks
an answer. The current model is not an answer verifier; the rejected scorer and
experimental grouping are not shipped. Do not use a lack of exact keyword overlap
as a failure by itself, or a related passage as proof of a complete answer.

The last verified full CI/inference run is [38500026388](https://github.com/muhannadnouri/papercut.io/actions/runs/38500026388) on `15b3710`; all ten jobs passed, including explicit cached-provider activation on desktop, Android emulator and iOS simulator. It predates the result-tab follow-up, so current-source full CI remains a separate gate. CI does not launch the WebView or replace this checklist. Recorded provider checks cover Small FP32/INT8, not every Base/Large device combination. Earlier history lives in [progress](search-v2-progress.md).

## Isolate the library first

A Git branch does not isolate application data. On Linux, close **all Papercut
instances** before launching an isolated test instance: the single-instance
plugin can otherwise route a second launch to the already running application.
Keep the disposable directory until results are recorded. Relaunch with the same
directory when checking persistence/restart behavior.

The native shipping build now includes the provider: use `npm run desktop` on
desktop, `npm run android:apk:native-tts` on Android, or `npm run ios:ipa:native-tts`
on macOS for iOS. No separate semantic build command is needed. Semantic search
still starts off until opted in. When resuming package smoke, exercise the built
installer/AppImage/APK/IPA as well as the library cases below.

The explicit unbundled source-build recipe remains available for diagnostics,
using the existing shared runtime and isolated app data:

The following recipe uses **Fish**, from the repository root:

```fish
set task_shared_dir "$PWD/src-tauri/tts/runtime/linux-x64-shared-libs"
env SHERPA_ONNX_LIB_DIR="$task_shared_dir" \
  ORT_LIB_LOCATION="$task_shared_dir" \
  npm run tauri -- build \
  --features native-tts-shared,experimental-search-embeddings --no-bundle

set task_smoke_dir (mktemp -d /tmp/papercut-smoke.XXXXXX)
mkdir -p "$task_smoke_dir"/data "$task_smoke_dir"/config \
  "$task_smoke_dir"/cache "$task_smoke_dir"/state
set task_library_path (string join : -- "$task_shared_dir" $LD_LIBRARY_PATH)
env XDG_DATA_HOME="$task_smoke_dir/data" XDG_CONFIG_HOME="$task_smoke_dir/config" \
  XDG_CACHE_HOME="$task_smoke_dir/cache" XDG_STATE_HOME="$task_smoke_dir/state" \
  LD_LIBRARY_PATH="$task_library_path" \
  ./src-tauri/target/release/app
```

This command is a Linux test recipe, not Android/iOS build validation. For a
normal build, use the existing desktop build command and the same isolated launch
environment. Confirm the starting library is empty rather than the everyday
library. An isolated app will need its own optional voice/model files; do not
move or damage the installed app's files.

## Search, evidence and reader navigation

- [ ] Search choices appear without a visible **Search Mode** heading. Check that
      the radio group retains its localized accessible name, both radio labels,
      arrow-key selection and the visible mode-dependent helper text. Check narrow
      English/Arabic views and a screen reader; helper/error associations remain intact.
- [ ] Import ordinary English and Arabic HTML/EPUB documents and a text PDF.
      Confirm import completion, library entries and reader text. Include a long
      document, repeated words and matches in different sections/pages.
- [ ] In **All words**, use a generic query such as `harbor electric timetable`
      after checking the actual source. Every included document must contain all
      required terms at document scope. Cross-section matches must say the terms
      were found in different sections. Unrelated starter documents must not
      appear from loose partial matching: `orchard lantern survey` must not count
      a document containing only `or`. Check a positive starter query such as
      `offline audiobook` and a stemming form such as `searches` as well.
- [ ] Keyword results form one list with one count, without Your Library/Starter
      Documents section headings. Bundled cards have a **Starter Document** badge;
      uploaded cards retain their existing evidence. When results are capped,
      **Showing N of M** distinguishes displayed cards from eligible matches.
      Include/exclude starter and uploaded documents through the existing filter;
      cards and counts must reflect the submitted scope.
- [ ] Switch the same query to **Broader · Your Library**. Partial matches must
      report coverage/missing terms honestly, and starter documents stay excluded.
      One/two-word and over-twelve-expression searches still require all words;
      relaxation applies only to 3–12 distinct term expressions. Switching modes reruns the
      submitted query and preserves the intended document filters.
- [ ] Check an authored quoted phrase, a missing phrase and a long pasted quote.
      Quotes remain mandatory in both modes. Straight/curly punctuation and
      supported PDF line-wrap/hyphen handling must retain the complete query and
      real source evidence. Include a joined word copied from a PDF Unicode
      hyphen wrap (U+2010/U+2011), in both All words and Broader, with semantic
      search off and on. Test a new import and an existing schema-11 library
      after the automatic repair; section/page navigation must still land on the
      source and highlight the complete phrase across the original line-wrap
      spans. In the PDF reader, Find the same joined phrase and its copied
      hyphenated spelling; check counts, next/previous navigation across repeated
      occurrences/pages, and clearing Find. Include a wrap after non-BMP text
      (such as emoji) to check UTF-16 offsets. Repeat target highlighting on an
      OCR page. Same-line compounds and paragraph breaks must stay intact;
      ordinary HTML/EPUB Find must retain its original matching behavior.
      An over-limit query should show a useful error.
- [ ] Check a request with thirteen distinct words whose final word distinguishes two sources.
      The final word must remain required in both modes. Inputs above 128 terms
      or 4,096 Unicode characters across unquoted text and phrases must show the
      inline limit error, never results for a silently shortened request. Shorten
      the input and retry; verify the error clears and filters remain selected.
- [ ] Try a generic typo such as `calender meeting agenda` against a document
      containing the correct wording. Any suggestion is an explicit whole-query
      retry that preserves the other unquoted words, search mode and filters.
      Quoted searches are excluded from suggestions; other suggestions may be
      absent under the conservative policy. There are no
      hard-coded synonyms or promised conceptual/spelling-variant matches.
- [ ] Check supported Arabic tashkeel/tatweel/alef normalization against authored
      text. Quotes still verify source wording; counts/highlights must not invent
      literal occurrences or change displayed source text.
- [ ] Open the result, a selected evidence passage, a concordance occurrence and
      next/previous in-document Find matches. Each should land at the selected
      source section/text/PDF page, including legacy imports where available.
- [ ] Reimport/replace one test document, change its title and delete another.
      Search and filters must reflect the new library without stale results.

## Model-cache setup

The earlier five-group smoke acceptance covered cache-only controls. The current
UI adds opt-in/indexing and conceptual results; those new cases below remain
unaccepted until tested. Downloading alone still does not enable semantic search.

- [ ] In a build without embedding features, Settings reports optional model setup unavailable.
      Browser settings do not offer native model controls. Ordinary search works.
- [ ] In the native shipping build, open **Settings → Semantic Search → Manage**. Opening
      settings must not download or load a model. Check the FP32/INT8 labels,
      disk reserve. **Semantic Search** appears before
      **Download Model / Manage Model**, then **Library Index** when applicable. Without files,
      the toggle is off/disabled with an accessible explanation pointing below.
      Open **Technical Details** to check device-independent resource/lifecycle
      guidance and conceptual scope/quoted-clause matching guidance; host RAM
      measurements and CI claims must not appear in the UI.
      Selecting INT8 shows its result/performance caveat beside the format choice.
      General Settings shows the observed state and **Manage**; **Back** returns
      there and the header **Close** icon closes Settings without changing consent.
- [ ] Download requires unchecked, per-format Hugging Face consent. Changing
      format clears it. Only proceed with a supported test format and sufficient
      disk space. INT8 on this development host is not mobile/other-CPU acceptance.
- [ ] Start a real download, close/reopen settings and confirm progress/busy state
      recovers. The compact header activity button should reopen the Search view
      from ordinary search and the reader. Cancel while pending; do not expect
      instant interruption. Controls remain busy until cleanup returns. Repeated clicks must not start extra jobs.
      No repeated closing-Settings notice appears. Header activity remains the
      route to progress/cancellation; continued work after app exit/suspension is not promised.
- [ ] While indexing, confirm the header shows a spinner and phase, with completed
      document counts only after native work starts. Counts are not remaining time
      or a job percentage. Pending cancellation has its own label. Settings remains
      available; activity opens the Search view by keyboard from home and reader.
      Check a narrow window, Arabic/RTL, dark mode and reduced-motion preference:
      no overlap with Back/title/Settings, visible focus and no animated spinner
      when reduced motion is requested. Listen with a screen reader for phase
      changes without repeated announcements on every document tick.
- [ ] Finish/cancel an index build after closing settings or opening the reader.
      The corresponding header outcome should last about eight seconds. An indexing
      failure should retain **Review**, with no busy spinner, until reviewed or a
      new action starts; opening it retains the diagnostic in Search settings.
      Retry and disable must not revive an old outcome. This is an in-process
      notice, not a restart history; test failure only in the disposable instance.
- [ ] Complete a download. Presence by size and successful verification are
      distinct messages. **Verify Cache** should reuse valid files offline rather
      than fetching again. It still requires explicit per-format consent because
      missing files may need downloading. Neither action changes ordinary search
      results. With semantic search off, file presence must not claim Model Loaded;
      successful verification must be distinguished from presence.
- [ ] In the disposable instance only, test offline failure, removal and retry.
      Where practical, interrupt the app during a download, then restart with the
      same isolated directory and retry; stale staging must not become a complete
      cache. A damaged/incomplete final cache needs explicit removal/redownload.
- [ ] Expand **Manage Model**, select **Remove Model Files**, then cancel its
      confirmation: files remain. Confirm removal, then reopen/restart. The
      library/search remains intact; cache removal does not remove a separately created native index.
- [ ] Check keyboard focus, labels, progress/error announcements, Arabic RTL,
      narrow-window layout and recovery after opening/closing settings repeatedly.
      Header/Back/Close stay accessible while the task body scrolls; keyboard Tab
      reaches every native disclosure, including **Manage Model**, **Library Index**
      and **Technical Details**, and Escape restores focus to the opener. Repeat at
      enlarged text/WebView zoom: navigation must remain reachable without
      horizontal scrolling; browser preview alone does not close this check.

## Audio and responsiveness

- [ ] Use the normal voice setup to listen to an English/Arabic preview and a
      reader excerpt, including the official Arabic preprocessing option. Verify
      audible speech, stop/pause/resume and reader highlighting.
- [ ] Save/play a short audiobook, restart, then export/import it into the test
      library. Verify saved metadata, playback, reader text/navigation and delete.
- [ ] During model download/verification and ordinary search, interact with the
      reader, Find, settings and audio. Record stalls, CPU/RAM peaks, slow cleanup
      or crashes. UI setup itself does not establish native activation memory
      recovery; weak-device/repeated-load/battery/platform checks remain separate.

## Record results and decide

Record build/commit, platform/device, isolated-data path, pass/fail/not-run for
these groups, reproduction steps and logs outside tracked files. Any crash needs
its own backtrace/core where available; the earlier native crash remains
unexplained despite successful bounded repetitions.

The user has separately authorized Stage 7 opt-in integration after reporting
this smoke pass. Lexical search stays the default; an enabled semantic path must
fall back visibly when unavailable/incomplete. Representative same-language
quality, index/query/toggle lifecycle, reference-device resource limits and each
advertised provider platform still need their own release evidence. The earlier
native crash is not explained by this successful cache-only UI check.

## Main-content preview follow-up — 2026-10-12

**Deferred.** After upgrading, explicitly rebuild the semantic index: the new
passage policy rejects older vectors and uses keyword fallback until rebuilding.
Re-import older EPUBs if their saved HTML lost authored note/navigation roles.
PDF upgrades reuse matching saved page geometry; unavailable geometry retains
all text conservatively.

- [ ] With semantic search **off**, use a generic guide containing body prose,
      a note and references sharing a keyword. The preview prefers matching body
      prose; a note-only term and a quoted note phrase still return the note.
      All Words/Broader counts and document filters remain understandable.
- [ ] With semantic search **on** and the rebuilt index ready, body passages
      take precedence over notes/navigation within each document. A required
      quote in a note still constrains document eligibility; its conceptual
      passage may be elsewhere, as the explanation states. Keyword quote
      evidence should still show the quoted note.
- [ ] Open each selected passage in HTML/EPUB/PDF and confirm its original
      section/page and authored text, including Arabic/CJK or emoji. Footnote
      links, tables and reader/audio text remain available.
- [ ] Restart during preparation, resume explicitly and verify coverage updates.
      Re-import/edit content and confirm stale coverage cannot return old vectors.
      No model download or semantic rebuild starts solely from opening settings.

## Natural-boundary follow-up — 2026-10-12

**Deferred.** Explicitly rebuild the semantic index after this upgrade; the
`chunks-v2` policy rejects prior windows. No model redownload is required.

- [ ] With semantic search **off**, keyword searches, quotes, filters and reader
      navigation behave as before. Enabling semantic search with an old index
      shows keyword fallback until the explicit rebuild completes.
- [ ] After rebuilding, inspect conceptual passages in ordinary English, Arabic
      and CJK prose containing several sentences or paragraphs. Prefer complete
      sentences and useful repeated context, and open the result at its original
      section/page. The card preview may still visually truncate a longer indexed
      passage; inspect the source before judging the boundary.
- [ ] Include a long sentence, an unbroken string, closing quotes, decimals and
      URLs. Preparation must finish or report an actionable error; forced cuts
      are allowed when input limits prevent a complete sentence. Body/auxiliary
      and page/section edges must remain distinct, with no fabricated joins.
- [ ] Repeat quote/filter/cancel/resume and restart checks after rebuilding.
      Record changed passage counts, preparation time and any missing context
      outside tracked files. Do not treat this mechanical check as relevance
      acceptance; separately grade whether the displayed text answers the query.

## PDF backmatter follow-up — 2026-10-12

**Deferred.** Upload schema 11 also refreshes PDFs already classified by schema
10, including wrapped index entries and parenthesized cross-references. This
upgrade queues existing PDF roles for recalculation from saved page geometry.
Explicitly update the semantic index afterward; no model redownload or unrelated
EPUB-only rebuild is required.

- [ ] With semantic search **off**, ordinary prose and numeric tables still have
      useful keyword evidence. Printed references, bare-domain citations and
      subject-index terms remain searchable, including quoted phrases. Include
      wrapped page-number entries and parenthesized cross-references in a generic
      guide, plus prose and numeric-column headings that must remain body content.
      Open their original pages and confirm reader text/locators remain intact.
- [ ] With semantic search **on**, affected PDFs show keyword fallback until
      explicit index preparation finishes. Afterwards, body passages take
      precedence over recognized references/indexes within documents containing
      both. A required quote in auxiliary text still constrains the document.
- [ ] Restart during preparation and explicitly resume. Coverage must recover
      without repeated role resets or a model download. Existing EPUB-only work
      should remain ready. An unknown layout may remain main content; do not
      interpret unrelated nearest passages as verified answers.

## Neighboring-context follow-up — 2026-10-13

**Deferred.** Rebuild the app; this context display itself needs no model download
or index rebuild. Complete any outstanding PDF-role/index update from the preceding
pass first. The jsdom regressions, older browser preview and native fixtures do not close these cases.

- [ ] With semantic search **off**, no conceptual cards or context controls appear;
      keyword results, All Words/Broader, quotes, filters and reader opening still work.
- [ ] Confirm **All Words** / **Broader** mode labels, with the imported-library
      scope still explained in Broader's helper text.
- [ ] With semantic search **on** and the index ready, expand **Passage & Nearby
      Text** using pointer and keyboard. Up to three bounded previews show the
      match and nearest available neighbour on each side, in source order. The
      match is emphasized; nearby text is not another relevance-ranked answer.
      **Show More Nearby Text (N)** reveals remaining bounded previews, and
      **Show Less** restores the nearest window with focus retained. Check a
      second card and changed match: expansion must not leak across them.
- [ ] Open the match and different nearby blocks in PDF and EPUB. Each opens its
      own original page/section and text target. Check a split qualification, a
      Unicode passage and a mixed body/note block; no omitted note is stitched into
      body text. A required quote elsewhere in the document need not appear here.
- [ ] Narrow the document filter, replace a query, disable semantic search or edit/
      remove a source while a query is running. Old context must disappear with its
      old card; fallback must preserve keyword search. Opening guards apply to every
      excerpt while a document is loading.
- [ ] Check narrow and wide layouts, RTL, long prose/code and text zoom. PDF
      layout breaks reflow while blank-line paragraphs remain; HTML/EPUB authored
      breaks stay intact. Unicode previews shorten safely with ellipses, six-line
      clamps and readable widths. No horizontal overflow; focus is visible and
      controls are reachable. Check viewport stability after More/Less, and
      collapse the disclosure to return to the compact card. Read full source
      through **Open Passage**; source offsets and reader highlighting must survive.

## Stage 7 preparation controls — new follow-up

**Deferred by the user on 2026-10-09.** The separate conceptual-results UI is now
functional. This is the requested reminder: the lifecycle cases below and the new
query cases that follow still need a rebuilt-app pass when the user is ready.
Earlier five-group acceptance does not cover these cases.

Rebuild with the existing native shipping command above and use
**Settings → Semantic Search → Manage** in a disposable profile; Developer mode is
not required. Shipping builds include the provider but start semantic use off. Keep the same profile for
restart checks; the explicit unbundled recipe is only a diagnostic alternative.

- [ ] Fresh profile: toggle starts off. Opening Settings or searching never
      downloads a model, activates one or starts indexing. Ordinary All words,
      Broader, quoted phrases and filters still work. Builds without embedding
      features report unavailability even if that profile previously saved an opt-in.
- [ ] Model choices: Small retains FP32 and INT8; Base and Large offer FP32 only.
      Choosing one starts no download, activation or indexing and resets download
      permission. Model/precision selectors lock while enabled or busy. Separate
      selectors share a row on wide screens and stack in narrow English/Arabic
      layouts. Base/Large show higher resource and explicit rebuild guidance.
- [ ] In a disposable profile, explicitly download a chosen additional model
      when hardware/storage allows, then enable and build the index. Base/Large
      total files are approximately 1.05/2.10 GiB, plus the installer reserve;
      disk size is not peak memory. Turn off and switch model/precision:
      incompatible coverage must not appear ready and searches must fall back
      to keywords until **Build Index** is explicitly requested. Confirm source
      documents and lexical search survive. Switching back after index replacement
      requires rebuilding again. Restart preserves selection and explicit opt-in
      without downloading or indexing. Removing one selected model's cache leaves
      other cached models intact. These cases remain deferred; local native probes
      do not replace WebView/device or relevance acceptance.
- [ ] Choose a cached model/format and turn on **Enabled** in the semantic-search panel. No
      download occurs; activation failure shows an error and lexical availability; submitted searches show fallback guidance.
      Retry after repairing the cache. Model/format/cache mutation requires turning
      off first. Turning off during activation stays off after cleanup.
- [ ] Once loaded, **Manage Model** collapses to the selected format and **Model
      Loaded** summary. Expanding it must not show a verify-before-use warning,
      download permission or installer action. Turn off: preparation controls
      remain accessible by expanding Manage Model, and explicit consent is still
      needed before invoking the installer. Installed unloaded files collapse to
      **Files Stored**; missing/incomplete files keep preparation expanded.
- [ ] With current nonempty coverage, **Semantic Search** and **Library Index**
      keep unnumbered headings. Ready/up-to-date status and document counts
      are visible in Library Index; the toggle reports **On** and there is no
      Build/Update action or nested Index Details. Passage counts remain supporting
      text in wide/narrow English/Arabic views, with visible keyboard focus. Partial coverage offers **Update Index**;
      inactive, failed or running states must not claim readiness.
- [ ] With semantic search off and no saved index, Library Index is hidden. With
      saved preparation/checkpoints, it is collapsed with **Saved Index · Search Off**;
      expand it to remove work without enabling search. On, pending cancellation,
      removal and errors keep the affected panel visible/expanded. Ready coverage
      must not be inferred from stored-work presence, including after a model change.
- [ ] There is no separate Manage Storage card. **Remove Model Files** is in model
      management and **Remove Index** is in Library Index, separated from routine
      preparation actions. In a disposable profile, cancel each confirmation and
      verify no deletion; then confirm. Empty model storage disables removal and
      reports **No model files stored**. Partial final/staging files remain removable.
      Cleared index work disables removal (or hides the empty off-index), even though
      SQLite tables/triggers remain. Repeat clicks must not reopen empty removal.
      Verify keyboard focus remains on the model summary or semantic toggle after
      confirmation and returns to the initiating removal button on cancel.
- [ ] General Settings and the semantic task view agree on observed state:
      off, loading, indexing/cancelling, index required, empty/checking library,
      ready, unavailable status or keyword-only fallback. The toggle reports On/Off;
      Library Index owns readiness and progress. Saved opt-in is not readiness. Loading or
      failed activation must not tell an already-enabled user to enable again;
      index actions wait for a loaded model. Opening either view starts no job.
- [ ] With no searchable uploads, the library-preparation panel requests imports,
      never reports `0 / 0` as ready and disables **Build Index**. With
      uploaded searchable text, click **Build Index** (or **Update Index** when
      compatible indexed work exists). Encoded/reused
      counts update and completed-document coverage grows. Close/reopen Settings
      or enter a reader during work: the job continues. Ordinary search and
      audio remain usable; record stalls, native errors and resource peaks.
      No repeated closing-Settings notice appears during indexing.
- [ ] During indexing, wide Library Index layout places **Passages Processed: n**
      and **Documents Indexed: x of y** across one row, with title-case English
      labels and no “This Run” suffix. The processed count remains job-local;
      document coverage can stay zero while the first document is in progress.
      A single divider precedes the shared **Stop Indexing** / **Remove Index**
      action row, at opposite ends. Stop stays neutral and removal stays disabled
      during work. Idle Build/Update uses the same action row; confirmation stays
      beneath it with Cancel/Remove and existing focus restoration. At 600px and
      below, both rows stack without overflow in English/Arabic RTL and light/dark
      themes. All controls retain readable labels and touch targets.
- [ ] Cancel during indexing, then resume. Cleanup can wait for the current
      native call. Completed/checkpointed work is reused; partial documents are
      not reported ready. Turn off during indexing: work stops, the model unloads,
      lexical search remains available, and late completion cannot enable it.
- [ ] Restart with the same profile while opt-in is on: cached activation returns
      without downloading or automatically indexing. Explicitly build/resume.
      Turn off, restart again: no activation or indexing. Cached files/checkpoints
      remain; do not expect the RSS to instantly return to its initial value.
- [ ] Edit a title, import/delete a document, accept recognized text or receive a
      library transfer; status reflects invalidation automatically even while idle.
      Reopening semantic settings refreshes status without starting work; build/update
      synchronizes it. **Remove Index** inside **Library Index** first asks for
      confirmation. Cancel leaves
      coverage intact; confirming clears derived coverage but preserves documents
      and lexical search.
      Build again; turn off and separately remove cache files. With a saved
      opt-in and missing cache on restart, activation fails visibly without a
      download; ordinary lexical search remains available.
- [ ] Check keyboard/assistive labels, status announcements, Arabic RTL and
      narrow-window layout for the new toggle/progress/actions. Repeat only the
      changed lifecycle cases; the prior passed smoke groups need not all be
      rerun without a new concern.

This follow-up does not establish conceptual relevance or Android/iOS/weak-device
readiness. Record pass/fail, build/device and any errors outside tracked files;
the unexplained earlier native crash remains a release concern.

## Stage 7 conceptual results — new focused cases

These remain deferred along with the new lifecycle cases, despite the passing
frontend/handler checks. Use sources you can inspect, not private queries in
tracked fixtures.

- [ ] Start off: search and switch All words/Broader. Only lexical results appear;
      no conceptual loading/fallback section, model activation or index job starts.
      Download/verify alone must not enable semantic search.
- [ ] Enable a verified cached model and finish **Build Index** or **Update Index**.
      Submit a question about an actual passage in the library, with different wording from that
      passage. **Conceptual Results** appears separately from
      the ordinary results count/cards, with no **Keyword Results** heading.
      The header contains only its title and shown-document count; no permanent
      imported-document explanation or **How Matching Works** disclosure appears.
      Matching guidance remains in Settings → Semantic Search → **Technical Details**;
      expand it with the keyboard and check wrapping/focus on narrow/RTL views.
      Check localized counts for zero, one and multiple displayed documents;
      complete indexing coverage is absent here and remains available in Settings.
      All words/Broader counts, marks and Explore Matches
      retain literal meaning. A document may occur in both independent channels.
- [ ] Conceptual cards show actual source text without query-word marks, literal
      occurrence counts or confidence percentages. Open an English and Arabic
      HTML/EPUB passage and a text PDF result. Check section/page/text targeting
      against the displayed passage, including a long block and an older import.
      Report wrong jumps or missing passage highlights; section/page fallback is
      not exact-span acceptance. Reader Find and audio highlighting still work.
- [ ] Apply include, exclude and empty scopes, including a scope containing only
      starter documents. Conceptual results never escape the uploaded scope.
      Counts mean shown documents, not an uncapped match total; partial-fallback
      coverage describes the current eligible scope.
      Switch filters/modes and rapidly resubmit A→B→A; no old card may reappear.
      While a conceptual query is still running, submit a different query/scope.
      Once it finishes, the latest request must complete without another submit;
      it must not stay in failure fallback merely because the old slot was busy.
- [ ] Try quoted-only and mixed queries, then a missing quoted clause. A title-only
      occurrence must not qualify; all quoted clauses remain required somewhere
      in the document before semantic selection. If clauses are in different
      sections, the displayed semantic passage may differ and must not pretend
      to be literal quote evidence. Check supported punctuation/hyphen aliases.
- [ ] Open an existing library with Unicode PDF hyphen wraps. The automatic repair
      updates derived lexical text from matching saved page layers, without
      downloading models or starting inference. Affected conceptual coverage
      becomes stale until an explicit **Update Index** / **Build Index**;
      lexical search remains available. Missing or mismatched page layers preserve
      existing text; re-import the PDF if those pages still cannot match.
- [ ] In a disposable library, add unindexed text or remove the derived index.
      Enabled search must show missing/partial fallback and recovery guidance,
      with lexical results available; partial conceptual cards stay hidden and
      **Documents ready for this search** shows ready/eligible coverage.
      Build/resume restores conceptual querying. A fully scoped ready empty
      response is separate from failure, not a model/index error.
- [ ] Turn off during activation/indexing/querying and immediately return to search.
      No conceptual card or fallback notice survives; late work cannot re-enable
      it. Edit a title/text, delete a document, remove/rebuild the index, then
      retry: no stale passage/title appears. Native calls can take time to stop.
- [ ] With a saved opt-in, open a build without embedding features or remove its cache before restart.
      Lexical search remains available; unavailable semantic search is explicit,
      and Settings lets you turn the saved preference off without a provider.
      Recovery never downloads automatically. Check a failed/over-token-limit
      query and explicit shorter retry without losing scope or phrases.
- [ ] Check keyboard/focus, polite loading/fallback announcements, Arabic RTL,
      narrow layout and reader opening busy states. Keep reader/audio interaction
      running during conceptual search/indexing and record stalls/crashes/resource
      peaks. This development-host smoke does not establish mobile/weak-device,
      thermal/battery or representative relevance acceptance.


## Settings Action Presentation Follow-Up

**Deferred.** Include these checks in the existing model/index setup round;
use the normal native shipping build and disposable data.

- [ ] Download/Verify and Remove Model Files occupy opposite ends of one row
      on wide views and stack without overflow on narrow/zoomed/RTL views.
      Removal confirmation appears below the row with the specific removal label;
      Cancel returns focus to the removal trigger. Stored-file/index and busy
      guards still prevent empty, repeated or active-model removal.
- [ ] Remove Model Files, Remove Index and their confirmations use the app's
      tonal button surface with red text, no colored border/fill, and a visible
      keyboard focus ring in light/dark themes. Disabled removal is muted;
      Cancel Download and Stop Indexing retain neutral secondary styling.
- [ ] An active transfer does not instruct you to remove incomplete files.
      Transfer/verification/removal and pending cleanup have appropriate status;
      100% transferred does not claim successful installation before verification.
      Reopening while another model/format downloads labels that actual download.
- [ ] Download and indexing progress retain Cancel Download/Stop Indexing without
      either background-work notice. Closing Settings keeps the job running;
      header activity remains visible and lets you reopen controls to cancel.
      Idle recovery copy returns after termination, with cancellation/failure
      details and no download/index activation on open.
- [ ] Library Index keeps its outer disclosure but has no nested Index Details.
      Coverage stays visible; a nonzero active passage count is supporting text.
      A new empty index does not repeat zero passages or No Saved Index.
      Remove Index remains a separate maintenance action, with its inline
      confirmation, busy and empty-index guards. Routine Refresh Status is absent;
      failed reads expose Retry Status. Retry clears only the status failure,
      preserves job errors and restores focus after successful recovery. Stale
      failures cannot overwrite newer snapshots. Build/Update remains the primary action.

- [ ] Enable Semantic Search is the stable toggle label when off or on; its
      checked state and Off/On status agree. Resource guidance is
      available in Technical Details without repeating it in the toggle card.
      During indexing/stopping, Library Index shows one app spinner and phase even
      when collapsed; reduced motion disables rotation. Its body shows completed
      documents and processed passages without repeating the phase, zero reuse or
      a disabled Build action. Nonzero reused work is visible. Index Required has
      an amber indicator and actionable help only when enabled with a loaded model
      and incomplete nonempty coverage. Reuse help requires known completed work.
      Stop Indexing remains neutral and completed checkpoints remain reusable.
- [ ] Download consent uses the app accent and visible keyboard focus. Clicking
      its label toggles consent without starting a download. Its row is at least
      44px tall; the checkbox aligns with the first text line in narrow/RTL layouts.
      New setup still starts unchecked, and unavailable controls stay disabled.
- [ ] Manage Model stays expanded after a download/verification completes in this
      visit. Manual expansion/collapse survives ordinary status refreshes; reopening
      Settings with stored/loaded files starts collapsed. Check pending work/errors,
      format changes and removal do not conceal required recovery feedback.
- [ ] Both removal actions and their confirmations show the existing trash icon
      beside a readable label. Opening confirmation hides the original removal
      trigger and focuses Cancel; cancelling restores it and returns focus.
      Only one removal action is visible for that object. Confirming still uses
      the specific removal label, preserves document data and checks busy/storage
      guards. Verify bookmark/audio removal icons retain their previous appearance.

## Settings Header Navigation Follow-Up

**Deferred.** Include in the existing rebuilt-app setup round; settings apply
immediately and these checks do not replace the model/index lifecycle cases.

- [ ] Root Settings shows only a header Close icon, with a localized accessible
      label/tooltip. Semantic setup and Legal & Notices add an icon-only Back arrow.
      Back returns one level; a legal notice returns to the list before root Settings.
      Close, Escape and backdrop dismissal close all Settings; reopening starts
      at root and keyboard focus returns to the opener. No bottom Back/Done row
      remains in these views.
- [ ] Scroll a long setup/legal view at narrow width and enlarged text/WebView zoom.
      Header controls occupy their own row above the title, matching the Settings
      cog's square size: 37.6px on desktop, 40px at narrow widths and at least 44px
      with a coarse pointer, at default text size. Icons remain 24px. They stay
      visible while only the body
      scrolls, titles wrap without overlap, and Tab/Shift+Tab remain inside the dialog.
      Check light/dark focus
      rings, Arabic RTL arrow/layout, and localized screen-reader labels.
- [ ] Closing or going Back during a download/index build leaves its header activity
      available; reopen to cancel explicitly. Preferences/consent are preserved.
      Edit/paste/document-info forms retain their Save/Cancel actions; confirmation
      dialogs retain explicit decisions.
- [ ] Open Audio Setup from both the reader and Audiobooks tab. It has only header
      Close, without footer Done/Back. Close/Escape/backdrop dismiss setup and restore
      focus; preferences remain saved and any parent-owned download keeps running.
- [ ] Transfer Library has header Back to Settings and Close to dismiss all Settings,
      without footer Back/Done. During export, import, preparation, receiving or
      waiting/sending, both controls are disabled and Escape/backdrop cannot dismiss
      the dialog. Explicit Change Selection/Stop Sending still cancel the session;
      navigation becomes available again once idle. Completion remains visible until
      Back or Close; reopening Settings starts at its root.

## Settings Safe Areas and Feedback Follow-Up

**Deferred.** Use a rebuilt artifact from the new commit; earlier reported smoke
does not accept these settings changes.

- [ ] On Android and iOS, expand every semantic-settings section and scroll to both
      ends at narrow width, enlarged text/zoom, portrait and landscape, with keyboard
      open/closed. Content and Back/Close stay within the system safe areas; only the
      navigable body scrolls. Check notches, status/gesture bars and actual WebView
      inset propagation. Ordinary forms, Transfer Library, Audio Setup and legal
      dialogs retain usable actions and existing dismissal/busy/focus guards.
- [ ] Enable semantic search on a slower device: Manage Model announces Loading
      Model once with a spinner even when initially collapsed. Off remains usable.
      Completion, activation failure/retry and disabling clear the spinner; a late
      activation completion cannot restore On. Download/verification remains a
      separate phase. Check reopening during native activation and restart.
- [ ] The toggle card has no redundant On line or duplicate loading label. Off,
      prerequisites, fallback and error/retry remain understandable. Main Settings
      still conveys readiness without a switch. Screen readers resolve descriptions
      and announce the loading phase without repeated indexing counters.
- [ ] How It Works shows Matching, Device Resources and Saved Work in light/dark
      and Arabic RTL. Check wraps and translated copy: partial relevance, document-wide
      quotes, local search/download connection, model resource costs, retained work,
      restart/update/retry and interrupted-download behavior remain clear.
- [ ] On a fine pointer, enabled Remove Model Files/Remove Index and their inline
      confirmation buttons show the app’s tonal hover with red labels; keyboard focus
      retains a red ring. Disabled removal during loading/indexing stays quiet.
      Coarse-pointer taps do not acquire a persistent hover appearance.
