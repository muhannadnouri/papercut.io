# Search Validation

[Search architecture and behavior](user-document-search.md) is the current technical
reference. This file owns evidence and runnable validation, not a development diary.
Expected use is one offline desktop/mobile user with hundreds of documents and long
books; one maintainer should be able to resume testing without replaying the branch.

## Acceptance And Scope

**2026-10-16: no remaining confirmed merge blocker for the agreed search-v2 scope.**
Keep optional semantic search default-off. Further smoke is deferred. iOS installed-app/
distribution acceptance is after merge and before its first release. Retained Library
Transfer diagnostics do not repair the reported connectivity failure; discovery,
stable-port and secure firewall work remain the separate
[transfer redesign TODO](library-transfer.md#deferred-lan-redesign--2026-10-15).

| Evidence | Source and limits |
| --- | --- |
| Verified full CI | [Run 38859560492](https://github.com/muhannadnouri/papercut.io/actions/runs/38859560492), attempt 1, `feature/search-v2`, exact source `e511c3991966637f21d4f4c482aa8d737c78e50a`: all 10 jobs passed. Logs confirmed actual Small FP32/INT8 provider index/query/lifecycle execution on Linux, Windows, Intel/Apple Silicon macOS, Android x86_64 emulator and iOS arm64 simulator. Packaging passed; unsigned Apple builds do not establish signed distribution. |
| CI suites and audit policy | Frontend 311 passed / 1 optional skipped; default Rust 196 passed / 5 ignored; provider Rust 254 passed / 10 ignored, with cached inference separately executed. npm audit reported zero vulnerabilities; Rust reported zero under the existing guarded inactive-rkyv waiver, with 11 nonblocking warnings. This is not a warning-free dependency graph. |
| Owner acceptance | Owner reports satisfactory WebView layout/Android behavior and conceptual results on a private corpus. Exact installed SHA/device/model and each recovery/accessibility case were not supplied; do not infer them from CI. This closes the reported merge concerns without claiming every checklist case passed. |
| Earlier reported subset | On 2026-10-14: CachyOS Linux partial smoke and Android 17 model download, enablement, explicit indexing and conceptual query, Small FP32. Device model/architecture, installed SHA and fresh/existing library were unconfirmed. iOS was not tested. Earlier cache-only acceptance is historical, not blanket lifecycle acceptance. |
| Experiment/fixture cleanup | `63226fa`: standalone experiment removed, generic encoded-space transfer URL retained, personal-book-dependent EPUB check replaced by a generated fixture. Shipping app code stayed identical to the CI source. Local frontend 311 passed / 1 skipped, default Rust 197 passed / 4 ignored, provider Rust 255 passed / 9 ignored; lexical evaluation/baseline and 7 mobile runner self-tests passed. One formerly ignored EPUB test now runs normally. No new inference/platform CI or device smoke was performed. |

Broader minimum-OS/physical-device coverage, weak hardware, all Base/Large choices,
battery/thermal behavior, exhaustive accessibility/recovery and signed distribution
remain unverified limits/follow-ups. They are not newly demonstrated defects or
universal-support claims. macOS signing/notarization stays in the existing
[Apple release procedure](apple-distribution.md). No further smoke case is marked
passed by consolidation, mocked DOM tests or native CI.

### Lasting Decisions And History

- Keyword ranking, Broader, Arabic search forms, explicit spelling retries and source
  evidence remain independent of semantic consent. No maintained synonym pairs.
- Conceptual results use their own view and ranking, not fusion with Keyword. Required
  quotes constrain documents, not necessarily the displayed conceptual passage.
- Small FP32/INT8 and optional Base/Large FP32 use one verified provider at a time.
  Large-model local execution is not cross-platform quality/resource qualification.
- Natural, role-aware passage windows and original source targets are shipped. The
  tested grouping/compact-window and BGE reranker/scorer routes failed their frozen
  quality/cost gates and are not integrated. No new model/candidate/thread sweep or
  ANN system is queued without a measured need.
- Keep actual-path lexical evaluation, generic fixtures, native/IPC regressions and
  mobile runner self-tests. The standalone experiment is retired, not maintained.

The final pre-consolidation records are archived at
[`63226fa`](https://github.com/muhannadnouri/papercut.io/tree/63226fa9b40a8601241029c923bf617a0e8a78b3/docs):
[development journal](https://github.com/muhannadnouri/papercut.io/blob/63226fa9b40a8601241029c923bf617a0e8a78b3/docs/search-v2-progress.md),
[branch reviews](https://github.com/muhannadnouri/papercut.io/blob/63226fa9b40a8601241029c923bf617a0e8a78b3/docs/search-v2-branch-review.md),
[provider research](https://github.com/muhannadnouri/papercut.io/blob/63226fa9b40a8601241029c923bf617a0e8a78b3/docs/search-embedding-provider.md)
and [original smoke cases](https://github.com/muhannadnouri/papercut.io/blob/63226fa9b40a8601241029c923bf617a0e8a78b3/docs/search-v2-smoke.md).
These dated snapshots preserve measurements, rejected options and fixes, not current
instructions or reopened gates. The standalone source/lockfile remains in
[history at ffbb4e6](https://github.com/muhannadnouri/papercut.io/tree/ffbb4e65ba8e77d80c2fd4df0db76fad4827d13f/experiments/search-v2-embeddings).
Keep private corpora, questions, grades, raw logs and downloaded weights outside Git.

## Automated Checks

From the repository root:

```sh
npm test
npm run lint
npm run check:i18n
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib
npm run test:search-eval
node scripts/check-mobile-provider-inference-selftest.mjs
```

`test:search-eval` checks the frontend parser and actual native import/store/search
path in a disposable SQLite database, then writes `dist/search-v2-evaluation.json`.
The immutable [original baseline](../scripts/fixtures/search-v2/baseline.json) is
machine input, not a current benchmark. The runner requires its query IDs to remain
and relevant baseline top-ten documents not to disappear. It does not assert identical
rankings or compare changing-corpus aggregate percentages as like-for-like quality.
The original 30-query baseline names its source/corpus hash; the current generic
[corpus](../scripts/fixtures/search-v2/corpus.json) adds cases. Never regenerate the
baseline merely to pass a regression. Assisted spelling retries are reported separately.

Optional production-path private evaluation opens a read-only consistent database
snapshot; inputs/reports must be outside the checkout and outputs new:

```sh
npm run test:search-eval -- --library /external/search.sqlite3 /external/queries.json /external/report.json
```

Use a consistent exported database if read-only SQLite WAL metadata is inaccessible;
do not copy a database during active writes. Private reports contain source excerpts
and identifiers. They are not independent relevance judgments. Review-packet modes
remain optional developer tools in the existing script.

### Provider And Native Runtime Checks

Prepare shared libraries with the existing platform build helpers. On Linux, after
runtime preparation (no model download implied):

```sh
task_shared_dir="$PWD/src-tauri/tts/runtime/linux-x64-shared-libs"
SHERPA_ONNX_LIB_DIR="$task_shared_dir" LD_LIBRARY_PATH="$task_shared_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --features native-tts-shared,experimental-search-embeddings
```

Providerless capability/IPC errors are covered by default Rust tests. Provider suites
cover fixed manifests, cache integrity, bounds, cancellation, panic cleanup, source/
vector validation, migration/invalidation and read-only retrieval. Tiny local HTTP
fixtures test status, size, checksum, truncated body, idle timeout, offline errors
and HTTPS policy without upstream model downloads.

Cached tests are ignored by default. With the same provider Cargo options, explicitly
select `search_embeddings::provider::tests::cached_provider_activation_switch_failure_and_unload`
and append `-- --ignored --exact --nocapture`. `PAPERCUT_EMBEDDING_TEST_MODELS` names
an external test-only cache with verified `float32/` and `dynamic-int8/` assets.
Optional `PAPERCUT_EMBEDDING_TEST_FORMAT` selects one of the four fixed selections;
Base/Large caches use that selection's named directory. `PAPERCUT_EMBEDDING_TEST_CYCLES`
accepts 1–20 (default 1) for repeated lifecycle checks. Only explicit test-only
`PAPERCUT_EMBEDDING_TEST_DOWNLOAD=1` permits the production installer to provision
missing pinned assets. These switches are not application settings.

The real test checks finite normalized vectors, distinct query/passage encoding,
input validation, cancellation/publication, disable/switch/failure cleanup and one
shared ORT environment. First-cycle disposable English/Arabic SQLite controls also
exercise actual tokenization/indexing, unchanged-source reuse, checkpoint resume,
required quotes, scope, reader evidence and query cancellation. Later cycles do not
repeat indexing. No installed library is used.

Linux-only Tauri MockRuntime tests exercise actual command macros/serialized IPC:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  search_embeddings::ipc_tests::command_capability_and_argument_contract

task_ipc_data=$(mktemp -d /tmp/papercut-cache-ipc.XXXXXX)
XDG_DATA_HOME="$task_ipc_data" PAPERCUT_EMBEDDING_IPC_TEST_DATA="$task_ipc_data" \
  PAPERCUT_EMBEDDING_TEST_MODELS="$CACHED_MODELS" \
  SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" LD_LIBRARY_PATH="$SHARED_LIB_DIR" \
  timeout 120s cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --lib \
  --features native-tts-shared,experimental-search-embeddings \
  search_embeddings::ipc_tests::command_cache_lifecycle_in_isolated_app_data -- --ignored --exact --nocapture
```

`CACHED_MODELS`/`SHARED_LIB_DIR` are explicit external-cache/prepared-runtime inputs.
The cached test refuses an ordinary data root, uses a separate app identifier and
copies rather than mutates the source cache. It checks pending activation/status,
slow cleanup, removal guards, explicit offline reuse and SIGKILL/restart recovery.
MockRuntime does not launch a WebView. The no-model process-interruption check is
`search_embeddings::cache::tests::process_interruption_recovers_staging_and_reuses_published_cache`;
it kills only owned disposable children at partial/staged/published boundaries.
These tests cover process death, not power loss or mobile suspension.

With provider features, `--lib native_tts::engine::preprocess::tests` checks actual
Arabic diacritization, invalid tensors and concurrency. The ignored
`native_tts::engine::synth::tests::cached_arabic_preprocessing_smoke` additionally
requires `PAPERCUT_ARABIC_TTS_MODEL_DIR` and `-- --ignored --exact --nocapture`.
The standalone atomic Unix runtime-copy regression needs no Cargo dependencies:

```sh
rustc --edition=2021 --test src-tauri/vendor/sherpa-onnx-sys/runtime_copy.rs -o /tmp/papercut-runtime-copy-test
/tmp/papercut-runtime-copy-test
```

Existing CI runs frontend/default/provider checks and shipping builds. Real model
inference requires manual `provider_inference=true`, with `validation_scope=full`
for all targets. Ordinary CI does not download embedding weights. Keep one shipping
build per target; no duplicate provider build family is needed. The mobile runner
requires both formats' lifecycle/index/query completion, rejects zero/partial test
success or abnormal native exit, and cleans only owned emulator/simulator fixtures.
Its self-tests use mocked tools, not real devices. Signing follows release procedures.

### Performance And Native Limits

- The release/native lexical probe uses 500 documents / 100,000 authored sections,
  including three 1,200-section documents. Development-host warm medians met the
  one-second broad / three-second exact-or-mixed targets, with no warm run over five
  seconds. Repeated synthetic text, warm OS caches and native-only timing do not
  establish real-book diversity or weak-device/WebView performance. Preserve complete
  counts, evidence targets and payload bounds when rerunning it. This used the
  existing external-input evaluator with a privately authored synthetic workload;
  no dedicated lexical-scale test/fixture is committed. Use its prepare-library and
  library modes with explicit external inputs, rather than a nonexistent test selector.
  All unscoped cases counted 500 documents / 100,000 sections and returned 50 cards;
  the narrow scope counted 12 documents / 5,346 sections. Complete occurrences, source
  targets and three-passage/twelve-bin bounds were verified; largest compact response
  was 179.38 KiB. These are native payload checks, not IPC/render latency measurements.
- The explicit no-model conceptual probe uses 200 documents / 100,000 synthetic
  384-dimensional vectors. Run the test below to check scoped counts, required/missing
  phrases, deterministic ties, encoding count and read-only access. Historical host
  scans were about 155 ms; common-phrase candidate reuse reduced one matched probe
  from 3,416.7 to 189.9 ms. This excludes inference/indexing and cold-disk/UI cost.
- Small repeated native reload checks reached about 1.8 GiB whole-process high water;
  post-disable retained RSS was around 650 MiB in the recorded Linux runs. Base/Large
  bounded local lifecycle checks peaked around 2.85/5.18 GiB. These are host/test
  measurements, not device requirements or steady query costs. Unload does not
  guarantee immediate RSS return; no allocator trimming hook was added.
- The original Linux native crash has no proven cause/backtrace. Atomic-copy control
  reproduced a separate mapped-file truncation SIGBUS, not that SIGSEGV. Subsequent
  bounded lifecycle/Arabic-audio/restart/release and cross-platform inference passed;
  they do not prove universal stability or weak-device resource safety.

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --offline --release --lib \
  document_uploads::semantic::store::tests::conceptual_query_scale -- --ignored --exact --nocapture
```

## Deferred Manual Checklist

**Not run by this consolidation.** Use one disposable profile and inspectable generic
sources across the rounds below. Each checkbox means the full case, including its
subconditions. Record pass/fail/not-run, build SHA, device/OS/architecture, model/format,
fresh/upgrade state, timings and safe reproduction details outside tracked files.
Owner acceptance above does not fill these boxes automatically. Resume only when
smoke is requested; iOS follows its explicit post-merge/pre-release deferral.

### Preparation And Isolation

Use the shipping commands in the [guide](user-document-search.md#ownership-and-shipping-builds)
and launch the built package, not just an unbundled source binary. A Git branch does
not isolate application data. Close all Papercut instances first: single-instance
routing can otherwise send the launch to an everyday profile. Keep disposable data
for restart checks; never remove or move installed library/model files to set up a test.

This optional source-build diagnostic recipe is **Fish on Linux**, from the root;
it does not validate an installer or mobile platform:

```fish
set task_shared_dir "$PWD/src-tauri/tts/runtime/linux-x64-shared-libs"
env SHERPA_ONNX_LIB_DIR="$task_shared_dir" ORT_LIB_LOCATION="$task_shared_dir" \
  npm run tauri -- build --features native-tts-shared,experimental-search-embeddings --no-bundle
set task_smoke_dir (mktemp -d /tmp/papercut-smoke.XXXXXX)
mkdir -p "$task_smoke_dir"/data "$task_smoke_dir"/config "$task_smoke_dir"/cache "$task_smoke_dir"/state
set task_library_path (string join : -- "$task_shared_dir" $LD_LIBRARY_PATH)
env XDG_DATA_HOME="$task_smoke_dir/data" XDG_CONFIG_HOME="$task_smoke_dir/config" \
  XDG_CACHE_HOME="$task_smoke_dir/cache" XDG_STATE_HOME="$task_smoke_dir/state" \
  LD_LIBRARY_PATH="$task_library_path" ./src-tauri/target/release/app
```

### Keyword And Source Evidence

- [ ] **L01 — Imports.** Import English/Arabic HTML/EPUB and text PDF, including long
  documents, repeated words and matches across sections/pages. Confirm completion,
  Library entries and reader text; keep footnotes, tables and authored links usable.
- [ ] **L02 — All Words and counts.** Every required term must occur at document scope;
  cross-section matches disclose that distinction. Check `harbor electric timetable`,
  starter substring exclusion (`orchard lantern survey` must not match only `or`),
  positive `offline audiobook` and stemming `searches`. One Keyword list/count has
  Starter Document badges, uploaded-first order and honest shown/available limits.
- [ ] **L03 — Broader.** Uploaded-only scope, honest missing-term coverage and fuller
  coverage ranking. Relax only 3–12 distinct words; 1–2 and >12 require all. Changing
  Keyword matching reruns the submitted query without losing filters. Do not confuse
  this with switching the independent Keyword/Conceptual views, which starts no query.
- [ ] **L04 — Quotes and PDF Find.** Check authored/missing/long/nested quotes, curly
  punctuation and ASCII/U+2010/U+2011 word wraps with semantic off/on and both Keyword
  modes. Use new imports and upgraded schema-11 PDFs; Find joined and copied-hyphenated
  spellings, repeated occurrences/pages, next/previous and clear. Include emoji before
  a wrap and an OCR page. Highlight the complete phrase at original UTF-16 spans;
  retain same-line compounds, paragraphs and ordinary HTML/EPUB Find behavior.
- [ ] **L05 — Complete query bounds.** Thirteen distinct words retain the distinguishing
  last word in both modes. Over 128 combined terms or 4,096 Unicode characters fails
  inline before starter/native/conceptual work. Shorten and retry without lost scope;
  changing/clearing input stops superseded starter lookups and stale publication.
- [ ] **L06 — Explicit spelling retry.** `calender meeting agenda` can suggest the full
  corrected query without claiming literal typo hits, including beside starter hits.
  Choosing it preserves other words/mode/filters. Quotes, Broader, names/identifiers,
  short or Arabic/mixed-script input must not invent suggestions or synonym promises.
- [ ] **L07 — Arabic forms.** Tashkeel/tatweel/alef normalization retains authored text,
  real quote verification, highlights/counts and source targets; no fabricated occurrences.
- [ ] **L08 — Reader targets.** Open primary/additional evidence, concordance occurrences
  and repeated Find matches in HTML/EPUB/PDF, including legacy imports and Arabic/CJK/
  emoji. Land on the selected original section/text/page, not merely its document.
  Reader/audio highlighting and footnote/table navigation remain usable.
- [ ] **L09 — Library mutation.** Reimport/replace, rename and delete disposable sources;
  queries, filters, counts and evidence reflect changes without stale text/title.

### Model Preparation And Activation

- [ ] **M01 — Off and unavailable.** Fresh profile starts off. Opening Settings/searching
  starts no download, load or index. Providerless builds expose unavailability, retain
  Keyword and allow disabling a saved opt-in. While off, no conceptual selector, cards
  or fallback notice appear. Browser preview has no native controls.
- [ ] **M02 — Settings structure and naming.** Root summary/Manage opens the task view;
  no introduction follows Semantic Search (Experimental). Semantic Search comes before
  Download Model/Manage Model and conditional Library Index. One clickable status/switch
  row has the fixed accessible name Semantic Search, changing descriptions and valid
  IDs. Off/loading/indexing/stopping/required/ready/error summaries agree with root;
  opt-in alone is not readiness. No separate Enable label or repeated status line.
- [ ] **M03 — Consent and storage information.** Hugging Face permission starts unchecked,
  resets on model/format changes and completed attempts, and never starts work by itself.
  Label-click toggles consent, with accent/focus, 44px row and first-line alignment in
  narrow/RTL views. Download Size groups the 64 MiB reserve beneath its value; muted
  information with an icon is distinct from explicit verified-success feedback.
- [ ] **M04 — Model choices and switching.** Small FP32/INT8, Base/Large FP32; selection
  starts no work and locks when enabled/busy. Wide selectors share a row and narrow/RTL
  stack. Check INT8 caveats and Base/Large memory/rebuild warnings without host RAM/CI
  claims in product copy. On suitable hardware, explicitly download/enable/build another
  model; incompatible coverage requires rebuild, switching back after replacement too.
  Preserve sources/Keyword and other models' caches. Record untested choices honestly.
- [ ] **M05 — Stored versus verified versus loaded.** Verify Cache reuses intact files
  offline with explicit consent; it can fetch missing files. Pending/failed/cancelled
  work never shows verified success. Successful preparation shows a success-colored
  check-circle and Model Files Verified
  without enabling/indexing; selection/reopening clears it. Stored unloaded files say
  Files Stored, loaded files Model Loaded. Size presence alone is never integrity proof.
- [ ] **M06 — Activation feedback.** Enable only cached files; failure offers retry and
  Keyword availability. Manage Model announces Loading Model once with a spinner even
  when collapsed. Off remains usable; completion/failure/disable clear activity, and late
  load completion cannot re-enable. Check reopening during load and restart on a slow
  device. Download/verification is a separate phase; only one model is resident.
- [ ] **M07 — Download progress and navigation.** Start an allowed download, close/reopen
  Settings or enter a reader; recover actual selection/progress/busy state via header
  activity. 100% bytes is not installation before verification. No repeated background
  notice, incomplete-file removal instruction during transfer or queued duplicate job.
  Closing Settings leaves work running, without promising app-exit/suspension survival.
- [ ] **M08 — Download cancellation.** Leading decorative circle-X, neutral tonal action,
  busy until cleanup; repeated clicks queue nothing. Download Cancelled stays inside
  model management without red error/external Retry. Re-download requires fresh consent.
  Check focus when progress disappears; actual cancellation errors stay actionable.
- [ ] **M09 — Failure and interruption.** In disposable data, test offline/connection
  failure and process interruption. Stale staging never becomes a complete cache;
  explicit retry restarts, and damaged final files need explicit removal. Close Settings
  before failure: Model Download Failed/Review retains the selected model/diagnostic
  across reopening and status retry. New allowed work clears it; successful retry does
  not activate/index. Real download/cancel/status errors and Retry stay in the model
  panel when available; initial setup failure still offers Retry without that panel.
- [ ] **M10 — Model removal guards.** Cancel confirmation preserves files; confirm removes
  only selected files/staging, not sources, another model or derived index. Reopen/restart
  to confirm. Empty storage reports no model files stored and disables repeated removal;
  partial/staging remains removable.
  Enabled/loaded model and conflicting jobs block mutation; handlers recheck guards.
- [ ] **M11 — Disclosure state and explanations.** Stored/loaded files start collapsed
  on reopen; pending/error/missing/incomplete states expose recovery. Current-visit manual
  expansion and post-verification expansion survive refreshes. Loaded view hides consent/
  installer warnings, restored by turning off. How It Works covers Matching, Device
  Resources and Saved Work: partial relevance, document-wide quotes, local search/model
  download, costs, retained checkpoints, restart and explicit recovery. Check translations.

### Index Lifecycle And Header Activity

- [ ] **I01 — Explicit activation and empty libraries.** Enabled cached search does not
  build automatically. Empty eligible library asks for imports, never 0/0 ready, with
  Build disabled. Missing files keep the switch off/disabled with decorative warning
  and readable Model Download Required description; after preparation it disappears
  without enabling. Loading/failure must not tell an already enabled user to enable again.
- [ ] **I02 — Off versus stored work.** Off/no index hides Library Index; checkpoints
  show collapsed Saved Index · Search Off with cleanup access. Enabled, cancelling,
  removing or error states expose the panel; saved work never implies compatible readiness.
- [ ] **I03 — Build/update and status.** Explicitly build nonempty data, update partial
  indexed coverage, hide Build/Update when fully current. Show compatible completed
  documents separately from processed/reused passages; first-document coverage may stay
  zero. No fake percent/time, duplicate phase/zero counters, nested Index Details or
  routine Refresh Status. Retry Status clears read errors only, retains job errors,
  ignores stale snapshots and restores focus. Known incomplete loaded coverage gets
  actionable Index Required warning; reuse guidance needs confirmed reusable work.
- [ ] **I04 — Index layout.** Wide Passages Processed/Documents Indexed rows and shared
  Build/Stop versus Remove row use one divider, title-case labels and no This Run suffix.
  At <=600px/zoom/RTL stack without overflow. Idle/running/removal/confirmation states
  retain readable targets; removals stay disabled during work and Stop stays neutral.
- [ ] **I05 — Cancel, resume and disable.** Busy guards persist through native cleanup.
  Confirmed Indexing Cancelled is normal Library Index feedback; completed checkpoints
  remain. Incomplete coverage offers Update, complete coverage remains ready. A real
  failure racing Stop remains error. Refresh/reopen/resume explicitly, reusing work;
  partial documents stay unavailable. Off stops work/unloads without late re-enable.
- [ ] **I06 — Restart.** With the same profile, saved opt-in reloads cached activation
  without download or automatic indexing; build/resume explicitly. Off survives restart
  without activation. Files/checkpoints remain, with no instant RSS-reclamation promise.
  Missing cache with saved opt-in fails visibly without downloading; Keyword still works.
- [ ] **I07 — Mutation and index removal.** Rename/import/delete, OCR acceptance and received
  transfer invalidate observed coverage while idle. Reopen/status reads start no job;
  explicit update repairs it. Cancel Remove Index preserves coverage, confirm clears only
  derived work, then rebuild. Empty removal is disabled/hidden though tables remain;
  cache removal and index removal are independent, preserving documents and Keyword.
- [ ] **I08 — Source failures.** Index diagnostics identify document ID/one-based section
  when known without title/query/passage content. Retry reuses committed checkpoints;
  confirmed cancellation is not wrapped into a contextual error.
- [ ] **H01 — Running activity.** From search and reader, keyboard opens Search settings
  through header activity during download/load/disable/indexing. Show native phase and
  completed-document counts only after work starts; cancellation has its own phase.
  No overlap with Back/title/cog in narrow/RTL/dark views; stable live announcements
  avoid per-document repetition, reduced motion stops rotation, Settings stays available.
- [ ] **H02 — Outcomes.** Complete/cancel after leaving Settings: indexing outcome lasts
  about eight seconds. Failure/unavailable final status retains Review without spinner
  until reviewed/new accepted action; opening preserves diagnostic. Retry/disable cannot
  revive old notices. This is process-local feedback, not restart history.

### Conceptual Results And Upgrades

- [ ] **C01 — Independent channels.** With ready coverage, submit a supported question
  using different wording from source. Keyword/Conceptual have independent results,
  counts and loading; one document may occur in both. Conceptual Results shows returned
  documents, not confidence, literal counts or full-library totals. Zero/one/plural
  localization works; healthy indexing coverage/matching explanations stay in Settings.
- [ ] **C02 — Scope and latest request.** Include/exclude/empty/starter-only scopes never
  widen uploaded eligibility. Rapid A→B→A and query/scope changes during native work
  eventually complete the latest request without another submit or permanent busy
  fallback; no stale cards/title/context can reappear.
- [ ] **C03 — Required quotes.** Quoted-only/mixed/missing clauses, punctuation/hyphen aliases
  and title-only negatives. All phrases must occur in document body, possibly different
  sections; shown conceptual text may be elsewhere and never poses as literal evidence.
- [ ] **C04 — Contextual empty/fallback.** Before submission, Keyword has the original
  two-line welcome/exact-phrase hint, ready Conceptual the short prompt. Distinguish
  checking/loading/indexing, empty library and model/status errors; unknown state is
  not Index Required. Missing/outdated warning is only icon/title above Search Settings.
  After submission, missing/incompatible/partial responses retain explanation/scoped
  ready/eligible coverage and withhold cards; ready-empty is not failure. Ready filtered
  coverage overrides global warning. Keyword remains usable; failures do not switch tabs.
- [ ] **C05 — Settings action.** Cog precedes Search Settings; warning/button stack centered.
  Open directly without starting work, Back to root, Close/Escape/backdrop restore opener
  or cog when setup removed it. Retain draft/query, filters, selected view, open evidence,
  batch limits and focus; check keyboard/coarse touch/narrow/RTL and announcements.
- [ ] **C06 — Nearby text and hierarchy.** Expand Passage & Nearby Text: match and nearest
  available neighbour each side remain in source order, not relevance order. Matched
  Passage is a stronger semantic heading with persistent logical-start accent/tonal
  surface; Nearby Text is secondary with equal block padding/border space. More reveals
  remaining bounded previews, Less restores nearest window and focus. Down/up decorative
  chevrons share click/touch area; labels/counts wrap. Expansion is per card and resets
  on changed match; check theme/RTL/zoom, heading navigation and nearby hover distinction.
- [ ] **C07 — Preview and source opening.** Open match/different neighbours after More in
  English/Arabic HTML/EPUB/PDF, long blocks and legacy imports. Preserve each original
  text/section/page, including a split qualification or mixed body/note span without
  joining omitted notes. PDF wraps reflow but paragraphs survive; non-PDF breaks remain.
  Unicode/word ellipses, six-line clamps and readable widths do not replace full reader
  targets. Long prose/code must not overflow. Reader Find/audio still work.
- [ ] **C08 — Invalidation during work.** Disable, change source/title, delete or rebuild
  while querying/opening. Old cards/context/fallback disappear as appropriate; late work
  cannot restore opt-in. Every opening action respects reader-busy guards; Keyword survives.
- [ ] **C09 — Recovery input.** Providerless saved opt-in, missing cache, failed or over-token
  query and shorter explicit retry retain scope/phrases. Recovery never auto-downloads.
- [ ] **U01 — Body and inline roles.** Generic prose before/after a footnote, navigation,
  bibliography and references: Keyword still finds note-only terms/quotes, conceptual
  prefers body when present; required auxiliary quotes still constrain documents even
  if primary conceptual evidence is elsewhere. Preserve footnote links/tables/reader/audio.
- [ ] **U02 — Existing library repairs.** Use disposable upgrade copies: schema-11 PDF
  roles/Unicode wraps and schema-12 inline-role repair. Matching geometry/saved source
  repairs derived data and invalidates affected vectors; explicit build/update restores
  coverage. Restart mid-repair/resume without repeated resets or model download;
  unrelated EPUB work stays compatible where unaffected. Missing/mismatched source keeps
  text/classification; reimport role-losing old EPUBs. Check printed/bare-domain references,
  wrapped page-number indexes, parenthesized cross-references, numeric columns/tables
  and prose that must stay body. Unknown layouts may conservatively remain body.
- [ ] **U03 — Window policy.** Old semantic policies reject coverage until explicit rebuild,
  without model redownload or lexical disruption. English/Arabic/CJK sentences/paragraphs,
  long sentences, unbroken strings, closing quotes, decimals and URLs keep natural
  boundaries where possible; hard bounds may force cuts but never cross role/page/source.
  Repeat quote/scope/cancel/restart; record counts/time/context gaps. Inspect full source
  beyond visually truncated previews; grade answer support separately from window mechanics.

### Layout, Dialogs And Shared Regression Checks

- [ ] **V01 — Search controls.** No visible Search Mode heading; Keyword matching retains
  its accessible group name, radio labels/arrows/helper/error association. When enabled,
  centered tonal tabs/options sit under query, options stack narrow, helper/examples
  precede shared filter. Keyword examples only on empty Keyword, fill without submission;
  Conceptual hides them. Magnifier/node icons precede labels physically left, direction
  stays automatic, minimum targets 36px desktop/44px any coarse pointer, content wraps
  with zoom. Off-state options/examples also stay centered; preserve an open filter.
- [ ] **V02 — Lists and sticky switching.** Only compact controls stick, below mobile safe
  areas; input/helper/filter scroll. Both channels start at ten and More adds ten without
  another query; exhausted actions do nothing. The decorative leading down-chevron shares
  the batch button's target and full label/count; it cannot take focus. Loading/failure
  omit misleading zero counts.
  Retain selected view across submissions/navigation and never switch on async completion.
  While mounted retain per-view evidence/window/scroll, including unequal list lengths,
  first visit and switching before stickiness; new submission/scope resets windows/place.
  Keyboard/touch, hidden-panel focus/live state, portrait/landscape/zoom and More/Less
  stability work. Opening after More uses original reader targets.
- [ ] **V03 — Header navigation and focus.** Root Close, setup/legal Back+Close; legal
  detail backs to list then root. Close/Escape/backdrop dismiss all, reopen at root and
  restore focus. No obsolete footer Back/Done. Header controls above wrapping title
  match cog sizing (37.6px desktop, 40px narrow, >=44px coarse; 24px icons at default
  text size). Tab/Shift+Tab stays contained, disclosures reachable. During download/index
  navigation leaves activity available and preferences intact. Forms retain Save/Cancel.
- [ ] **V04 — Safe areas and appearance.** Android/iOS: expand all sections, scroll both
  ends with keyboard open/closed, narrow/zoom/portrait/landscape/notches/status/gesture
  bars. All four insets protect header/body, only body scrolls, no horizontal overflow.
  Repeat light/dark, English/Arabic RTL, long legal/setup views, reduced motion and actual
  screen-reader announcements. Tonal sections keep readable fields/focus/warning contrast;
  ordinary forms, audio, transfers and legal dialogs retain existing guards.
- [ ] **V05 — Removal and action styling.** Wide routine/removal actions sit opposite,
  narrow/zoom/RTL stack. Red removal labels/trash icons and tonal hover/focus/confirmations
  differ from neutral Cancel Download/Stop Indexing. Disabled controls do not hover;
  coarse touch has no stuck hover. One removal trigger per object; confirmation hides it,
  focuses Cancel, cancel restores trigger/focus, confirm uses specific label and current
  guards. If a panel disappears, focus reaches surviving summary/switch. Bookmark/audio
  removal icons retain previous appearance.
- [ ] **V06 — Adjacent dialogs.** Audio Setup from reader/Audiobooks has header Close and
  no footer Done/Back; dismissal restores opener and keeps parent-owned work/preferences.
  Transfer Library Back/Close and Escape/backdrop remain blocked through export/import/
  preparation/receiving/waiting/sending; explicit Change Selection/Stop Sending cancels.
  Idle navigation returns, completion stays until Back/Close, reopening starts root.
- [ ] **A01 — Speech.** English/Arabic voice preview and reader excerpt, including official
  Arabic preprocessing: audible speech, stop/pause/resume and correct reader highlighting.
- [ ] **A02 — Audiobook round trip.** Save/play short audio, restart, export/import into
  disposable library; verify metadata, playback, reader targets/highlighting and deletion.
- [ ] **A03 — Resources and relevance.** During download/verification/load/index/query,
  use reader/Find/Settings/audio; record stalls, CPU/RAM, slow cleanup and crashes with
  backtrace where available. Repeat loads on suitable reference hardware; separately mark
  minimum-OS/weak-device/Base/Large/thermal/battery checks not run. Judge whether actual
  source supports questions (including unsupported ones), not similarity or keyword
  overlap alone. Nearest results can be irrelevant/incomplete; no-answer detection is
  uncalibrated. Desktop/native or emulator success is not physical-device acceptance.

## Consolidation Validation — 2026-10-16

The baseline is relocated byte-for-byte; only its evaluator path changes. Current
contracts replace stale provider-free shipping, old labels and duplicate acceptance
narratives. Unique cases from the 93 original checkboxes and 15 focused bullets are
reconciled into the workflow checklist above. History links pin the prior revision.
This consolidation changes no application/runtime/dependency source or CI workflow.
Validation passes: the real lexical evaluator runs the parser regression (**1 passed /
1 optional skipped**) and native `search_v2_evaluation` (**1 passed**), including its
baseline retention assertions. The moved baseline matches the old file byte-for-byte
(SHA-256 `865765051584ea2b03b76d3304b9b69da41d3c8a2f55771ee20c02b922209981`).
Local links/anchors, archived Git objects, retired-path references, 108-to-51 case
reconciliation, script syntax/lint and diff checks pass. Application code, fixtures
other than baseline location, dependencies and workflows remain unchanged. No fresh
full suites/platform CI, model inference or installed-app smoke were rerun, and no
push, merge or release is implied.
