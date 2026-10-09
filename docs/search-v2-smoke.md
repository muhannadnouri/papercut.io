# Search v2 consolidated smoke pass

Status: **user-reported passed on 2026-10-09** for all five groups in the
consolidated normal/experimental-cache checklist. The user also reports full CI
passed. Exact build SHA, CI run URL and per-case logs were not inspected. The
checklist remains below for future regressions; new index/toggle/query behavior
added after this pass needs its own acceptance. Automated native, command and frontend checks are recorded in
[progress](search-v2-progress.md); this checklist does not mark them as manual
acceptance. Use documents you can inspect and keep private sources, questions,
screenshots and judgments out of tracked files.

Stage 7 native scale and cached-handler acceptance checks now exercise the
integrated query contract, including title invalidation, removal and disable.
They do not mark any interactive case as passed. The new lifecycle and
conceptual-results groups below remain explicitly **deferred** by the user;
run them together against the rebuilt provider-feature app when ready.

## Isolate the library first

A Git branch does not isolate application data. On Linux, close **all Papercut
instances** before launching an isolated test instance: the single-instance
plugin can otherwise route a second launch to the already running application.
Keep the disposable directory until results are recorded. Relaunch with the same
directory when checking persistence/restart behavior.

For the provider prototype, build explicitly using the existing shared runtime;
no separate Tauri configuration or development script is required:

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

- [ ] Import ordinary English and Arabic HTML/EPUB documents and a text PDF.
      Confirm import completion, library entries and reader text. Include a long
      document, repeated words and matches in different sections/pages.
- [ ] In **All words**, use a generic query such as `harbor electric timetable`
      after checking the actual source. Every included document must contain all
      required terms at document scope. Cross-section matches must say the terms
      were found in different sections; unrelated starter documents must not
      appear because a query was accidentally shortened.
- [ ] Switch the same query to **Broader · Your Library**. Partial matches must
      report coverage/missing terms honestly, and starter documents stay excluded.
      One/two-word and over-twelve-expression searches still require all words;
      relaxation applies only to 3–12 distinct term expressions. Switching modes reruns the
      submitted query and preserves the intended document filters.
- [ ] Check an authored quoted phrase, a missing phrase and a long pasted quote.
      Quotes remain mandatory in both modes. Straight/curly punctuation and
      supported PDF line-wrap/hyphen handling must retain the complete query and
      real source evidence. An over-limit query should show a useful error.
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

- [ ] In a normal Tauri build, Settings reports optional model setup unavailable.
      Browser settings do not offer native model controls. Ordinary search works.
- [ ] In an explicit provider build, open **Settings → Optional search model**. Opening
      settings must not download or load a model. Check the FP32/INT8 labels,
      file-size/RAM distinction, disk reserve and INT8 compatibility caveat.
- [ ] Download requires unchecked, per-format Hugging Face consent. Changing
      format clears it. Only proceed with a supported test format and sufficient
      disk space. INT8 on this development host is not mobile/other-CPU acceptance.
- [ ] Start a real download, close/reopen settings and confirm progress/busy state
      recovers. Cancel while pending; do not expect instant interruption. Controls
      remain busy until cleanup returns. Repeated clicks must not start extra jobs.
- [ ] Complete a download. Presence by size and successful verification are
      distinct messages. **Verify cache** should reuse valid files offline rather
      than fetching again. Neither action changes ordinary search results.
- [ ] In the disposable instance only, test offline failure, removal and retry.
      Where practical, interrupt the app during a download, then restart with the
      same isolated directory and retry; stale staging must not become a complete
      cache. A damaged/incomplete final cache needs explicit removal/redownload.
- [ ] Remove model files, then reopen/restart. The library/search remains intact;
      this cache-only UI does not remove a separately created native index.
- [ ] Check keyboard focus, labels, progress/error announcements, Arabic RTL,
      narrow-window layout and recovery after opening/closing settings repeatedly.

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

## Stage 7 preparation controls — new follow-up

**Deferred by the user on 2026-10-09.** The separate conceptual-results UI is now
functional. This is the requested reminder: the lifecycle cases below and the new
query cases that follow still need a rebuilt-app pass when the user is ready.
Earlier five-group acceptance does not cover these cases.

Rebuild with the existing experimental flags above and use **Settings → Optional
search model** in a disposable profile; Developer mode is not required. Normal
builds deliberately omit the provider. Keep the same profile for restart checks.

- [ ] Fresh profile: toggle starts off. Opening Settings or searching never
      downloads a model, activates one or starts indexing. Ordinary All words,
      Broader, quoted phrases and filters still work. Normal builds report
      unavailability even if that profile previously saved an opt-in.
- [ ] Choose a cached format and **Enable semantic search (optional)**. No
      download occurs; activation failure shows an error and lexical availability; submitted searches show fallback guidance.
      Retry after repairing the cache. Format/cache mutation requires turning
      off first. Turning off during activation stays off after cleanup.
- [ ] With uploaded searchable text, click **Build / resume index**. Encoded/reused
      counts update and completed-document coverage grows. Close/reopen Settings
      or enter a reader during work: the job continues. Ordinary search and
      audio remain usable; record stalls, native errors and resource peaks.
- [ ] Cancel during indexing, then resume. Cleanup can wait for the current
      native call. Completed/checkpointed work is reused; partial documents are
      not reported ready. Turn off during indexing: work stops, the model unloads,
      lexical search remains available, and late completion cannot enable it.
- [ ] Restart with the same profile while opt-in is on: cached activation returns
      without downloading or automatically indexing. Explicitly build/resume.
      Turn off, restart again: no activation or indexing. Cached files/checkpoints
      remain; do not expect the RSS to instantly return to its initial value.
- [ ] Edit a title, import/delete a document, or update extracted text; **Refresh
      status** reflects invalidation, and build/resume synchronizes it. **Remove
      index** clears derived coverage but preserves documents and lexical search.
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
- [ ] Enable a verified cached model and finish Build / resume. Submit a question
      about an actual passage in the library, with different wording from that
      passage. **Conceptual results · Your Library** appears separately from
      **Lexical results**. All words/Broader counts, marks and Explore matches
      retain literal meaning. A document may occur in both independent channels.
- [ ] Conceptual cards show actual source text without query-word marks, literal
      occurrence counts or confidence percentages. Open an English and Arabic
      HTML/EPUB passage and a text PDF result. Check section/page/text targeting
      against the displayed passage, including a long block and an older import.
      Report wrong jumps or missing passage highlights; section/page fallback is
      not exact-span acceptance. Reader Find and audio highlighting still work.
- [ ] Apply include, exclude and empty scopes, including a scope containing only
      starter documents. Conceptual results never escape the uploaded scope.
      Counts mean shown documents; coverage describes the current eligible scope.
      Switch filters/modes and rapidly resubmit A→B→A; no old card may reappear.
- [ ] Try quoted-only and mixed queries, then a missing quoted clause. A title-only
      occurrence must not qualify; all quoted clauses remain required somewhere
      in the document before semantic selection. If clauses are in different
      sections, the displayed semantic passage may differ and must not pretend
      to be literal quote evidence. Check supported punctuation/hyphen aliases.
- [ ] In a disposable library, add unindexed text or remove the derived index.
      Enabled search must show missing/partial fallback and recovery guidance,
      with lexical results available; partial conceptual cards stay hidden.
      Build/resume restores conceptual querying. A fully scoped ready empty
      response is separate from failure, not a model/index error.
- [ ] Turn off during activation/indexing/querying and immediately return to search.
      No conceptual card or fallback notice survives; late work cannot re-enable
      it. Edit a title/text, delete a document, remove/rebuild the index, then
      retry: no stale passage/title appears. Native calls can take time to stop.
- [ ] With a saved opt-in, open a normal build or remove its cache before restart.
      Lexical search remains available; unavailable semantic search is explicit,
      and Settings lets you turn the saved preference off without a provider.
      Recovery never downloads automatically. Check a failed/over-token-limit
      query and explicit shorter retry without losing scope or phrases.
- [ ] Check keyboard/focus, polite loading/fallback announcements, Arabic RTL,
      narrow layout and reader opening busy states. Keep reader/audio interaction
      running during conceptual search/indexing and record stalls/crashes/resource
      peaks. This development-host smoke does not establish mobile/weak-device,
      thermal/battery or representative relevance acceptance.
