# Search v2 consolidated smoke pass

Status: **prepared, not run**. The user has deferred interactive checks to one
consolidated pass. Automated native, command and frontend checks are recorded in
[progress](search-v2-progress.md); this checklist does not mark them as manual
acceptance. Use documents you can inspect and keep private sources, questions,
screenshots and judgments out of tracked files.

## Isolate the library first

A Git branch does not isolate application data. On Linux, close **all Papercut
instances** before launching an isolated test instance: the single-instance
plugin can otherwise route a second launch to the already running application.
Keep the disposable directory until results are recorded. Relaunch with the same
directory when checking persistence/restart behavior.

For the provider prototype, build explicitly using the existing shared runtime;
no separate Tauri configuration or development script is required:

```sh
# Set this to the existing verified shared-runtime directory.
SHARED_LIB_DIR="/absolute/path/to/existing/shared/libraries"
CARGO_NET_OFFLINE=true SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" \
  ORT_LIB_LOCATION="$SHARED_LIB_DIR" \
  npm run tauri -- build \
  --features native-tts-shared,experimental-search-embeddings --no-bundle

task_smoke_dir=$(mktemp -d /tmp/papercut-smoke.XXXXXX)
mkdir -p "$task_smoke_dir"/data "$task_smoke_dir"/config \
  "$task_smoke_dir"/cache "$task_smoke_dir"/state
XDG_DATA_HOME="$task_smoke_dir/data" XDG_CONFIG_HOME="$task_smoke_dir/config" \
  XDG_CACHE_HOME="$task_smoke_dir/cache" XDG_STATE_HOME="$task_smoke_dir/state" \
  LD_LIBRARY_PATH="$SHARED_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
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

The current UI only prepares model files; it does **not** activate the provider,
create a semantic index or enable conceptual results.

- [ ] In a normal Tauri build, Developer-mode settings report setup unavailable.
      Browser settings do not offer native model controls. Ordinary search works.
- [ ] In an explicit provider build, open **Settings → Developer mode**. Opening
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
      there is no semantic index or activation preference to claim removed here.
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

The next decision is acceptance evidence, not another speculative search feature.
Passing smoke alone does not authorize Stage 7: independently judged
representative relevance, compatible index/lifecycle/fallback behavior,
reference-device resource limits and every advertised platform still need their
own evidence. Keep hybrid ranking on hold or record a no-go if those gates fail.
