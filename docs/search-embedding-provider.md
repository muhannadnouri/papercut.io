# Experimental search embedding provider

This is an app-level, default-off **optional conceptual-search path**, with
separate **Keyword / Conceptual** result views with bounded document-card lists.
Functional Settings controls do not require Developer mode. The native shipping
scripts now compile the provider; model loading/querying still require user opt-in.
There is no automatic download or indexing. Saved opt-in reloads a verified cached
model on restart; without it, no activation or semantic querying occurs. Queries
never initiate installation/indexing, and lexical results finish independently.
Missing/incomplete/failed semantic work shows fallback guidance only while opted in.

The user reports partial downloaded-artifact smoke on CachyOS Linux and Android 17 with Small FP32, including Android model download, enablement, explicit indexing and conceptual search. The [consolidated smoke record](search-v2-smoke.md) separates these reported paths from unconfirmed cases. iOS was not tested. The new result tabs postdate that artifact; further app/device/accessibility smoke stays deferred. Earlier normal/cache reports remain historical, and relevance/resource caveats are unchanged.

The [branch review on 2026-10-10](search-v2-branch-review.md) is followed by
[corrections](search-v2-progress.md#stage-7--branch-review-corrections--2026-10-10)
for latest-request recovery and older-app lexical projection invalidation, with
regressions for both. Native platform checks do not replace these hook/migration
checks or deferred interactive acceptance. The [shipping integration](search-v2-progress.md#stage-7--provider-in-shipping-builds--2026-10-10)
now includes provider capability in existing native package scripts. Shipping
platform CI passes in the acceptance below; signing/installed-app acceptance
remains open, with user opt-in off by default. The separate historical compatibility crate is removed: use the
app-native checks under [Runnable checks](#runnable-checks), with the production adapter and
shared initializer. Its four-order results remain historical Git evidence; other
retrieval experiments stay frozen outside the product/CI graph.

Desktop-first validation is sequencing, not a desktop-only product restriction.
The commands and state are shared Rust code with no desktop-only availability
check. Android and iOS provider build/link checks now pass CI; runtime, lifecycle
and resource tests remain required before availability can be advertised.

## Shipping commands — 2026-10-10

| Target | Existing command | Provider/runtime |
| --- | --- | --- |
| Linux, Windows, macOS | `npm run desktop` | `native-tts-shared,experimental-search-embeddings`; existing bundled shared runtime |
| Android arm64 | `npm run android:apk:native-tts` | Same dynamic provider; existing staged JNI/ORT libraries |
| iOS signed export, on macOS | `npm run ios:ipa:native-tts` | `native-tts-ios,experimental-search-embeddings-linked`; existing static archives/signing |
| iOS unsigned simulator/device validation, on macOS | `npm run ios:ci -- --native-tts` / `npm run ios:ci:device` | Same linked provider; simulator remains archive-only |

These are the existing release/CI entry points, with no new npm command, config or
Cargo default. No embedding weights are bundled or downloaded by the build;
Settings download, opt-in and indexing remain explicit. Fresh or invalid saved
preferences start disabled, cache presence is not consent, and lexical search
remains the default. Preserving an earlier valid opt-in retains restart behavior.

`desktop:static` and Android/iOS builds without their native-TTS flag remain
explicit alternatives without the provider. Android's fully custom argument list
still owns its feature selection; include both shared features for a custom native
shipping build. iOS extra features retain the required linked shipping features.
The name of the existing Cargo feature is retained to avoid another migration;
compilation does not imply model activation or release acceptance.

Linux Debian/RPM/AppImage package build/content checks are recorded in progress.
The AppImage media check passes. Relocated payloads resolve packaged sherpa/ORT
without development library-path overrides, include provider code/license, and
omit embedding weights. These checks do not install or launch the app. Windows,
macOS and Android packages build/upload successfully in
[full run 38107442282](https://github.com/muhannadnouri/papercut.io/actions/runs/38107442282)
on `e686960`; unsigned iOS simulator/device builds also pass. Optional FP32/INT8
index/query and lifecycle checks pass on all six tested runtime targets, including
Android x86_64 emulator and iOS arm64 simulator. This does not establish installed
package, signed IPA/notarization, physical-device or minimum-OS acceptance.
The [CI consolidation](search-v2-progress.md#stage-7--shipping-ci-acceptance-and-consolidation--2026-10-10)
removes the now-redundant provider app builds while retaining shipping builds,
default/provider Rust checks and explicit inference.
[Full run 38110242011](https://github.com/muhannadnouri/papercut.io/actions/runs/38110242011)
on `1f020d3` confirms the simplified workflow, including both formats' native
index/query/lifecycle controls on all six runtime targets. Ordinary CI still
downloads no embedding weights. The [final branch review](search-v2-branch-review.md#final-acceptance-review--2026-10-10)
accepts automated checks while retaining deferred installed-app, quality/resource
and signed-distribution acceptance. No additional tuning or Stage 8 work is queued.

The sections below retain historical build/run evidence. Earlier “normal build”
claims refer to the lexical-only configurations used at those recorded sources.

## Build and capability boundary

The [current acceptance review](search-v2-progress.md#stage-7--verified-full-ci-acceptance--2026-10-09)
binds successful full CI run 38002904236 to pushed source `022ea95`, which includes
the integrated Stage 7 candidate. The earlier SHA gap is resolved. Normal
frontend/native checks and desktop/Android/iOS builds pass; these configurations
omit embedding features. The all-features dependency audit is not provider
runtime coverage. Prior local Linux provider checks remain separate evidence.
Desktop provider initialization/inference and bounded Android x86_64/iOS
simulator indexing/query checks are now verified below. Android arm64 execution,
physical/reference-device resources, representative quality, actual provider
shipping packages and explicitly deferred interactive smoke remain open.

The [provider CI follow-up](search-v2-progress.md#stage-7--opt-in-provider-ci-checks--2026-10-09)
now adds a locked Linux non-test library build and regression suite with
`native-tts-shared,experimental-search-embeddings` to the existing checks job.
It reuses Cargo's prepared sherpa/ORT libraries through the existing runtime-copy
helper. The steps run after default Rust tests whenever `code_checks` runs;
normal package jobs retain their original features. Local checks and
[full run 38005733746](https://github.com/muhannadnouri/papercut.io/actions/runs/38005733746)
on pushed `9bfe030` pass: remote non-test provider compilation and **216 tests
(10 ignored)**. The added CI steps take about 94/24 seconds respectively,
including compilation; these are not app search/inference timings.
Cached-model inference and asset/scale checks remain explicit: ordinary CI does
not download embedding weights or establish provider device/inference acceptance.

The [explicit runtime follow-up](search-v2-progress.md#stage-7--explicit-provider-runtime-checks--2026-10-10)
adds a default-off manual CI input, `provider_inference`. Dispatch
`validation_scope=desktop` with `provider_inference=true` after pushing to run
the existing real-model regression on Linux, Windows x64 and macOS Intel/Apple
Silicon. The test uses each host's prepared runtime libraries, isolated temporary
assets and the production verified installer. Both formats require about 594 MiB
on a cold cache; an intact cache is verified without fetching. Ordinary CI still
skips this test and downloads no embedding weights. Local Linux FP32/INT8 debug
and release runs pass. The
[desktop runtime acceptance review](search-v2-progress.md#desktop-provider-runtime-acceptance-verified--2026-10-10)
verifies successful run `38075038695` on pushed `982e3f9`: Linux, Windows x64,
macOS Intel and Apple Silicon each complete FP32/INT8 activation, inference,
disable and failure/cancellation checks. Windows now reaches the harness after
explicit Common Controls v6 manifest embedding; the repeated
`STATUS_ENTRYPOINT_NOT_FOUND` incident is closed on the tested runner.
The correction uses Tauri's default manifest and public build API, with no
runtime/dependency upgrade or upstream fork. Normal Windows packaging also passes.
The earlier native crash and allocator-retention concern remain separate gates.

The test executes one cycle per format and validates normalized finite vectors,
English/Arabic inputs and input limits using MockRuntime; it does not establish
semantic relevance, integrated app search/indexing or audio behavior. Whole-test
times are 45.37 seconds Linux/debug, 11.93 Windows/release, 11.20 Intel/release
and 12.08 Apple Silicon/release, including asset installation/verification and
lifecycle, excluding compilation. These are not app latency benchmarks.
Bounded Android emulator and iOS simulator provider inference now pass. The
[mobile inference harness](search-v2-progress.md#stage-7--mobile-provider-inference-harness--2026-10-10)
now adds manual Android **x86_64/API 35 emulator** and iOS **arm64 simulator**
checks. After pushing, dispatch `validation_scope=mobile` with
`provider_inference=true`. Linux installs/verifies the two pinned test caches
once and shares a one-day test-only artifact. Targets read temporary assets,
verify them again and execute the same exact ignored library regression; target
downloads are disabled. The Android test uses the prepared x86_64 shared runtime;
iOS uses the prepared simulator static archives and linked provider feature.
The runner rejects partial-format or zero-test success and cleans only its owned
emulator directory/simulator. [Run 38090360569](https://github.com/muhannadnouri/papercut.io/actions/runs/38090360569)
on `955fe5b` passes both formats on Android API 35/x86_64 (21.06 seconds) and
iOS arm64 simulator (16.26 seconds), with normal process exit and owned cleanup.
The Linux shared-check job also repeats both formats (44.64 seconds). These
whole-test times exclude compilation/boot and are not query benchmarks.

The Android-only initializer holds one shared ORT environment reference for the
process lifetime without retaining model sessions. It also applies to Arabic
preprocessing at first native use. Dependencies and upstream source remain
unchanged. The prior post-test destroyed-mutex abort does not recur with the
guard; its exact call chain remains unconfirmed without a native backtrace.
Retain the guard until a compatible upstream teardown fix is adopted and
validated. The runner captures the crash buffer on failure and continues to
reject nonzero exit after passing assertions. See the
[acceptance review and limits](search-v2-progress.md#mobile-provider-inference-acceptance-review--2026-10-10).
The [index/query extension](search-v2-progress.md#stage-7--native-indexingquery-checks-without-new-ci-jobs--2026-10-10)
now adds disposable English/Arabic SQLite fixtures to the first cycle of the same
native regression for each model format. Actual passage tokenization/encoding,
checkpoint cancellation/resume, unchanged-source reuse, scoped/quoted read-only
retrieval, source evidence, query cancellation and disable are checked alongside
unchanged production lexical results when the semantic path is unavailable.
Both formats now pass in [run 38094061234](https://github.com/muhannadnouri/papercut.io/actions/runs/38094061234)
on `f42af57`: Android API 35/x86_64 release/shared (**18.42 seconds**), iOS arm64
simulator release/linked (**28.39 seconds**) and Linux debug/shared (**46.67
seconds**). Both index/query and lifecycle markers are present, the exact native
test passes, and mobile runners exit normally with successful owned cleanup.
These are whole-test durations, not query latency or platform comparisons.
Run `38090360569` predates this index/query coverage. See the
[bounded acceptance review](search-v2-progress.md#mobile-indexingquery-acceptance-verified--2026-10-10).
The mobile runner additionally requires `index/query checks complete: Float32`
and `index/query checks complete: DynamicInt8`; temporary fixtures live beneath
its owned cleanup directory. No CI job, input, build variant or script is added.
These shared backend checks do not exercise mobile IPC or the React fallback
banner. The Linux-only cached IPC checks remain separate. No additional runtime
patch or model sweep is required by this result.
Android arm64 execution, minimum-OS coverage, physical-device
resource/lifecycle and quality acceptance remain open. Normal packages still
omit the provider, and interactive smoke stays deferred. No new npm command is
required; the workflow compiles and invokes the test runner directly.

The [cross-platform follow-up](search-v2-progress.md#stage-7--cross-platform-provider-build-checks--2026-10-09)
adds provider app-build checks to the existing Windows, macOS Intel/Apple Silicon,
Android arm64 and iOS arm64 simulator/device jobs. [Full run 38009770905](https://github.com/muhannadnouri/papercut.io/actions/runs/38009770905)
on `0796645` passes the Windows, both macOS and Android provider steps. Only iOS
fails: the provider simulator compiles/links, then its export collides with the
normal simulator output; the provider device step is skipped. The subsequent
[mobile run 38023521669](https://github.com/muhannadnouri/papercut.io/actions/runs/38023521669)
on `771b034` passes normal/provider simulator archives, both unsigned device
links and Android builds. The export failure is resolved. Desktop checks use
`--no-bundle`; Android builds a debug APK after
the normal APK upload. Normal artifact uploads precede these checks, and no
provider artifact is uploaded. Both iOS checks use linked ORT and unsigned
Xcode builds. No embedding weights are downloaded by these build checks.

The historical explicit commands for reproducing those added checks are:

| Host/target | Command |
| --- | --- |
| Windows/macOS, after the normal desktop build prepares runtime resources | `npm run tauri -- build --features "native-tts-shared,experimental-search-embeddings" --no-bundle` |
| Android arm64, with the existing SDK/NDK setup | `npm run android:apk:native-tts -- android build --apk --debug --target aarch64 --features native-tts-shared,experimental-search-embeddings` |
| iOS simulator, on macOS with Xcode | `npm run ios:ci -- --native-tts --features experimental-search-embeddings-linked` |
| Unsigned iOS device link, on macOS with Xcode | `npm run ios:ci:device -- --features experimental-search-embeddings-linked` |

The device helper preserves its required native TTS/frontend features when
additional features are supplied. The simulator helper already passes both
feature arguments; the pinned [Tauri 2.11.4 CLI](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-cli/src/mobile/ios/build.rs#L57-L59)
appends repeated `--features`. `ios:ci` now uses `--archive-only`, retaining
archive compilation/linking while avoiding the simulator export/rename collision.
The corrected iOS archive and provider device build are verified in the mobile
run above; release IPA commands still export normally. Running a provider build reuses local output paths;
that earlier pass built/uploaded lexical-only packages first. Current native shipping
scripts include the provider; the duplicate checks are removed after the successful
shipping/inference run recorded above.
Build/link success is separate from provider runtime initialization, installed
model inference and real-device resource acceptance.

- Builds without the provider feature omit FastEmbed entirely. `search_embedding_status` reports
  `compiled: false`; activation fails recoverably and disabling remains valid.
- `experimental-search-embeddings` adds the provider and dynamic ORT loading,
  selected by the shared-runtime desktop/Android shipping scripts.
- `experimental-search-embeddings-linked` adds the provider without dynamic
  loading, for the linked runtime configuration used by iOS. Choose this rather
  than the dynamic feature for an iOS build. A linked initializer test on Linux
  is not an iOS static-archive or device test.
- FastEmbed 7.1.0 enables ORT API 24 only in provider builds. Ordinary TTS retains
  API 17 when built without the provider. Both consumers use one shared initializer and ORT `rc.13`; official
  Libtashkeel remains unchanged. The existing packaged runtime must support the
  requested API. No runtime download or replacement occurs during activation.
  Initialization failures are cached; repairing a packaged runtime requires a restart.

`compiled` means that the provider is present in the build, not that a platform
has passed release validation. Status also returns `platform`, `activeFormat`,
`loadingFormat`, `downloadingFormat` and `encoderKey`. A non-null active format means integrity checks,
model construction and a finite normalized 384-dimensional warm-up vector passed.
It does not establish relevance quality or resource suitability.

The [normal packaging review](search-v2-progress.md#normal-packaging-and-platform-readiness--2026-10-08)
verifies a Linux Debian build without embedding features, including packaged
runtime bytes and relocated library resolution. Existing desktop/Android/iOS CI
jobs retain normal TTS package builds and now also include the separate provider
build checks above. Windows/macOS/Android provider builds and the corrected iOS
simulator archive/device links pass. Packaged provider runtime/device checks
and app smoke remain open.

## Explicit cached-model activation

The native `search_embedding_configure` command accepts `format: null` to
cancel/unload, or `"float32"`, `"dynamic-int8"`, `"base-float32"` or `"large-float32"` to activate an already staged
snapshot. It accepts no arbitrary path, model URL or unverified model identifier.
All asset reading and native loading/cleanup run on the blocking pool. Disabling
also signals index cancellation before waiting on provider cleanup and cancels
a pending model download; busy slots remain occupied until workers finish.
Status and download cancellation also wait for provider locks on the blocking
pool. Cooperative cancellation cannot interrupt a native inference call.
Native disable generations invalidate queued activations before dispatch and
are checked before loading and publication, so an early disable cannot be
undone by an older queued loader.

The fixed cache layout beneath Tauri's application data directory is:

```text
models/search/multilingual-e5-small/
  614241f622f53c4eeff9890bdc4f31cfecc418b3/
    float32/ or dynamic-int8/
      config.json
      tokenizer.json
      special_tokens_map.json
      tokenizer_config.json
      onnx/model.onnx
```

Small's existing paths and serialized selections remain unchanged. The
[pinned upstream snapshot](https://huggingface.co/intfloat/multilingual-e5-small/tree/614241f622f53c4eeff9890bdc4f31cfecc418b3)
uses `onnx/model.onnx` for FP32. The tested INT8 artifact is specifically
`onnx/model_qint8_avx512_vnni.onnx`, staged under the common `onnx/model.onnx`
filename. Its portability to other CPUs is unverified; do not assume that this
export is the mobile choice. The model card declares MIT licensing and describes
English `query: ` / `passage: ` prefixes, including for non-English text.

The [provider](../src-tauri/src/search_embeddings.rs) pins each asset's byte size
and SHA-256 and checks the actual bytes supplied to the loader. Installed sizes
are **487,352,505 bytes** for FP32 and **135,430,819 bytes** for INT8, including
all four tokenizer/configuration files. Disk size is not peak RAM usage.
No weights are bundled in the application or copied into the repository.

### Optional model capacities — 2026-10-11

The user requested Base and Large alongside the existing Small choices. Settings
separates **Model** from **Model Format**: Small retains FP32 and INT8; Base and
Large currently offer FP32 only. Semantic search still starts off. Selection
starts no download, activation or indexing; download consent resets on a change.
Turn search off before changing the selected model or precision. Only one model
can be resident. Each choice has its own verified cache; removing one does not
remove another.

| Selection | Installed bytes | Vector dimensions | Pinned snapshot |
| --- | ---: | ---: | --- |
| Small FP32 (`float32`) | 487,352,505 | 384 | `614241f622f53c4eeff9890bdc4f31cfecc418b3` |
| Small INT8 (`dynamic-int8`) | 135,430,819 | 384 | Same Small snapshot |
| Base FP32 (`base-float32`) | 1,127,143,136 | 768 | [Base snapshot](https://huggingface.co/intfloat/multilingual-e5-base/tree/d128750597153bb5987e10b1c3493a34e5a4502a) |
| Large FP32 (`large-float32`) | 2,252,993,226 | 1,024 | [Large snapshot](https://huggingface.co/intfloat/multilingual-e5-large/tree/3d7cfbdacd47fdda877c5cd8a79fbcc4f2a574f3) |

The [asset manifest](../src-tauri/src/search_embeddings/models.rs) pins sizes and
SHA-256 values. Base/Large use `models/search/<model>/<revision>/float32/`, with
the same four tokenizer/configuration files and `onnx/model.onnx`. Large also
requires `onnx/model.onnx_data` (2,235,363,328 bytes); the existing FastEmbed public
external-initializer API receives its verified bytes. The graph alone is not an
installed model. No new runtime, package fork or dependency is introduced.

Index callers read the active encoder identity and dimension together. The
identity covers all assets, including external weights. Small identities are
unchanged; a different model/precision has zero compatible coverage until an
explicit build replaces the derived index. Source documents and lexical search
remain intact. Returning to an older model requires another explicit rebuild
if its index was replaced; the app retains one derived semantic index.

Real Base and Large provider lifecycle checks passed locally with fully verified
assets: warm-up, finite normalized 768/1,024 vectors, repeatability, English/Arabic
input, prefix/size limits, unload/failure handling and the existing SQLite
index/query controls (filters, required quotes, cancellation/checkpoints and
incompatible-index fallback). The debug test process peaked at approximately
**2.85 GiB for Base** and **5.18 GiB for Large**, completing the bounded lifecycle
tests in 18.78 and 36.56 seconds. These are development-host process peaks and
test durations, not steady query latency, phone requirements or a relevance win.
Settings warns about the larger resource cost without promising hardware fit.

Existing CI remains on the Small FP32/INT8 controls. Base/Large execution on other
platforms, physical-device responsiveness and representative ranking quality are
not established by this Linux check. Interactive smoke remains deferred.

Only one activation can run. Re-selecting the active format is idempotent;
selecting another requires explicitly disabling first. A rejected selection
leaves the active provider intact. This avoids simultaneous resident embedding
models and a hot-switch rollback mechanism. After disabling, a failed new
activation leaves the provider disabled and ordinary search remains available.

Cancellation is checked during asset reads, before and after native setup, and
before publishing the session. Native session construction itself cannot be
interrupted; a cancelled session is discarded when setup returns, and its loader
slot remains occupied until cleanup finishes. Disable releases the model/session,
not the shared ORT environment used by other consumers. Releasing the provider
does not guarantee that the process allocator immediately returns memory to the OS.
Native cleanup serializes
state access briefly; responsiveness still needs device measurement.

Inference uses mean pooling, one text per call for the retained dynamic-INT8
policy, and at most two ORT intra-op threads on desktop / one on mobile. Inputs
require an E5 prefix and nonempty content; inputs exceeding 4,096 characters
including the prefix or 512 actual tokens including special tokens fail rather
than truncate. This is a provider bound, not a change to ordinary query parsing.
`encoderKey` includes snapshot, exact ONNX checksum, format, pooling, prefix,
dimensions, token limit and per-text policy. The new app index additionally
includes every tokenizer/config/model checksum and versioned extraction,
authored-source normalization and 320-token/40-overlap passage policy. Passage
content hashes include embedded metadata/body; database generations invalidate
source edits. The provider status key alone is not the index compatibility key.

## Explicit app indexing

`document_uploads/semantic.rs` owns jobs/commands; its `store.rs` owns only derived
SQLite tables and invalidation triggers. The lexical import/search code is not
modified. Four registered commands accept no source path, URL or arbitrary model:

| Command | Contract |
| --- | --- |
| `search_semantic_index_status` | Reports job state, compatible completed-document coverage and passage/checkpoint counts. No model activation or semantic-table creation. Without an active compatible provider, available semantic coverage is zero. |
| `search_semantic_index_build` | Requires a verified active provider; initializes compatible storage only on explicit request, indexes one document at a time and reuses unchanged validated passage vectors. It returns when the job finishes or fails. |
| `search_semantic_index_cancel` | Requests cooperative cancellation; the job remains reserved until the worker exits. Completed checkpoints survive for retry. |
| `search_semantic_index_remove` | Shares the indexing job slot, removes derived vectors and invalidates old writers, preserving source/lexical data and future invalidation tracking. Does not delete model files. |

The optional tables are created lazily in the upload database. Source mutations
and readiness invalidation are transactional even when a later normal build
edits the library. Partial documents stay unavailable until all current source
spans are synchronized; completed other documents remain available. Delete,
reimport/title/page/source edits, removal and identity changes prevent stale
publication. Explicit incompatible builds replace derived vectors; unknown
schemas fail closed. Stored vectors have independent checksums plus dimension,
finite-value and unit-norm checks. No write lock spans inference.

There is no automatic indexing resume. The React opt-in is persisted as described
below; enabled queries use completed compatible sources, while index retry is
explicit and reuses durable checkpoints. Native encoding and section
tokenization temporarily hold the provider lock; cancellation is checked between
calls, and status/disable may wait for a call to finish. Source retention is one
document at a time, not the whole library. These bounds still need device
measurement before release.

The new ignored cached-command test is explicitly run with FP32 and INT8 in
disposable data. It uses generic authored English/Arabic/Mandarin controls for
UTF-8 token spans/overlap, real encoding, completed-build skip, title invalidation,
removal/rebuild and deletion. The test uses the existing
`PAPERCUT_EMBEDDING_IPC_TEST_DATA`, `PAPERCUT_EMBEDDING_TEST_MODELS` and optional
`PAPERCUT_EMBEDDING_TEST_FORMAT` variables. It refuses an ordinary data root and
never downloads models. Structural language checks do not establish retrieval
quality.

For eventual feature removal, remove this module's handler/state registration,
the narrow provider indexing methods and the later React setting/search hook.
Existing installations also need a migration that drops the five
`upload_semantic_*` triggers before the three derived tables. Do not drop upload,
source or lexical tables. No generic plugin framework is introduced.

## Explicit downloads and cache removal

The feature-gated native commands provide the backend for opt-in prototype
settings. There is no automatic download. Downloading never activates the
provider, persists an opt-in preference or changes ordinary search.

| Command | Contract |
| --- | --- |
| `search_embedding_cache_status` | Returns the two fixed formats, `expectedBytes` and `filesPresent`. Presence checks exact file sizes and regular files, not fresh checksums. Activation always verifies actual bytes again. |
| `search_embedding_download` | Accepts only the existing `format` enum. Downloads the five checksum-pinned assets from the fixed upstream revision, verifies staging and promotes the complete directory. A verified existing cache is reused without an HTTP request. |
| `search_embedding_cancel_download` | Requests cancellation without unloading another active provider. Status remains busy until the worker returns. |
| `search_embedding_remove` | Removes only the selected model-cache directory and its interrupted staging directory. Removal is idempotent; pending activation/download must finish first, and an active selected model must be disabled first. Derived index removal is a separate explicit command; the later opt-in UI must coordinate both. |

No command accepts a URL, filesystem path or arbitrary model identifier. The
download and loader share one asset manifest and the same bounded streaming
size/SHA-256 checker. The two existing encoder identities are unchanged. Production
requests are HTTPS-only, including redirects. The upstream INT8 path remains the
AVX512/VNNI-named export; downloading it does not establish portability.

Before writing, the installer reuses library transfer's free-space preflight:
the full payload plus **64 MiB** of reserve on the same filesystem. Files are
written beneath a sibling `<precision>.installing`
directory, synced, rechecked, then renamed into place. Failed/cancelled work
cleans staging on normal return or Rust unwinding. After a process interruption,
the next explicit download discards stale staging and restarts; removal can also
clear it. There are no resumable transfers or automatic retries. A corrupt or
incomplete final cache is preserved and must be explicitly removed before
redownloading; a failed attempt never replaces an existing final cache.

One download or activation can run at a time in the app process. Downloading may
coexist with a previously active provider, but another activation or cache removal
is refused until download cleanup finishes. Cancellation and final promotion are
serialized with the same state lock: cancellation before promotion prevents
publication; cancellation after a committed install does not undo a verified
cache. This is app-owned cache coordination, not a multi-process cache service.

The blocking HTTP client uses the existing reqwest dependency, with 10-second
connection/idle-read deadlines and a 30-minute request deadline. Cancellation is
checked between streamed reads and during verification; an in-flight network
operation can take time to stop under those deadlines. No instant-cancellation
guarantee is made. The shared fetch boundary supplies the timer context needed
by reqwest's idle timeout. Recoverable HTTP/offline, size, checksum and filesystem
errors leave lexical search available.

`search-embedding-download-progress` emits camelCase `format`, `phase`,
`downloadedBytes` and `totalBytes`. Phases are `starting`, `downloading`,
`installed`, `cancelled` and `failed`; byte updates are throttled to roughly
two-percentage-point steps. Installed completion reports the full payload size,
including when an existing cache was verified. The settings panel consumes these
events. The earlier cache-only UI smoke is user-reported passed; the new lifecycle
controls require the focused follow-up below.

## Optional model setup

Open **Settings → Semantic Search → Manage** in a Tauri build; Developer mode is
not required. General Settings contains a summary rather than model/index actions.
The dedicated task view starts with **Semantic Search**, followed by **Download
Model / Manage Model** and **Library Index**. The switch remains off and unavailable
without model files; its status/prerequisite explanation is associated accessibly.
Downloading and building/updating the index remain separate explicit actions.
General Settings and the toggle card use the same observed readiness summary:
off, loading, indexing/cancelling, index required, empty/checking library, ready or
keyword-only fallback. Saved opt-in alone never means ready; inactive encoders,
failed jobs, index removal and empty coverage cannot claim readiness.
**Technical Details** holds device-independent resource, recovery and lifecycle
explanations. **Remove Model Files** lives inside model management; **Remove Index**
lives inside the library-index panel. Each has a separated destructive-action row,
consequence text and inline confirmation; there is no separate Manage Storage card.
The view reuses the existing dialog/focus handling, widens on
desktop and scrolls its body without hiding Back/Done on narrow screens.
Search view rules live in `SearchModelSetup.css`; header activity rules live in
`SearchActivity.css`, rather than general `AppSettings.css`.

Native shipping scripts compile the provider; providerless diagnostic builds
report unavailability. There is no desktop-only UI check. FP32 remains the
initial format, with installed-file size and the 64 MiB disk reserve visible.
INT8 result/performance differences and FP32 recovery guidance appear beside the
selected format. Host-specific RAM measurements and CI acceptance evidence stay
in developer documentation, not product copy. Expanded **Index Details** places
passage counts and **Refresh Status** across a row when space permits and stacks
them on narrow screens. Closing-Settings guidance appears only beside an active
download or indexing operation, with the header progress/cancellation route;
it does not promise work continues after app exit or mobile suspension.
CI emulator/simulator inference does not establish physical-device acceptance.

An unchecked consent box names Hugging Face and this device as the destination.
Each format change and completed attempt clears consent. Only an explicit
**Download Model Files** or **Verify Cache** click invokes the native installer.
The latter verifies a valid existing cache without fetching, but can fetch
missing files, so it still requires explicit per-format consent. Metadata-only
presence is not labeled as verified. Installed files collapse to **Files Stored**,
including when search is off; missing/incomplete files and relevant model operations
remain expanded. A loaded selected provider has verified its
assets during activation; the view shows **Model Loaded** in a collapsed **Manage
Model** summary and hides consent/installer controls. Turn off to restore them;
this UI status does not add a persistent verification record. **Remove Model Files**
is available for incomplete final files or interrupted staging, but unavailable
when neither exists. Native `hasStoredFiles` is separate from complete-by-size
`filesPresent`; cleanup remains idempotent. Confirmation and handlers recheck their
current guards, and status refresh disables repeated empty removal. Active
selected caches cannot be removed, and pending native activation/download blocks
cache mutation. Download/verification never activate. The separate semantic-search
opt-in panel owns activation; turn it off before changing format or removing
model files. Ordinary search results remain unchanged.

When off, an empty Library Index is hidden. Saved derived preparation/checkpoints
remain in a collapsed **Saved Index · Search Off** panel with cleanup access.
Native `hasStoredIndex` is independent of active-encoder coverage: it includes
prepared identity/partial work, excludes empty tables retained after removal, and
does not claim indexed readiness. When enabled, working, removing or reporting an
index error, the panel expands; errors remain visible after disable. Active status
and coverage share a wrapping row. Confirm/cancel returns focus to a surviving
control before the confirmation or empty off-index disappears.

A compact activity button beside Settings remains visible during native download,
model loading/disable or indexing, including reader navigation. Its spinner and
phase label open the existing Search settings view; the Settings gear remains
available. Observed running indexing shows completed-document coverage, not a
job percentage or time estimate. A stable live region announces phases without
repeating every coverage tick, and narrow reader layouts keep activity below Back.
App-owned indexing completion/cancellation notices last eight seconds; failure
or unavailable final status offers **Review** until opened or another accepted
action starts. Reviewing retains the settings diagnostic. Only the confirmed
native cancellation result is treated as cancellation; late outcomes after disable
are ignored. These notices survive navigation within the process, not restart.
The App-owned semantic controller listens to existing download start/terminal
events, skips byte-progress events and polls only while busy. It starts no work
from this notification. Empty libraries request imports rather than reporting
zero coverage as readiness, and disable **Build Index** until documents exist.

The panel subscribes before reading status, polls only during pending work and
recovers native busy/cache state when reopened. Closing settings, changing tabs
detaches listeners and timers; it does not cancel the
native download. Cancellation remains busy until the worker finishes, and is
separate from unloading an active provider. Errors are shown with retry and
explicit damaged-cache removal guidance. The panel requires the App-owned
semantic controller; selection and failed-download diagnostics survive Settings
navigation in that controller. Local state handles cache preparation/progress,
consent and confirmation only, with no standalone model-manager mode. An
observed cancellation event supplies a terminal message even when the panel did
not start the job. There is no persistent job/error history across process
restart; verification remains an explicit action.

Frontend regressions use the repository's hook-mocking pattern and
server-rendered controls to exercise default-build availability, explicit
consent/format resets, duplicate-click rejection, pending cancellation,
native-state recovery after reopening, unrelated progress, active-cache removal
refusal, successful verification, failure/retry and delayed listener cleanup:

```sh
npm test -- src/components/AppSettings/SearchModelSetup.test.tsx
```

These checks do not replace WebView/app IPC, actual downloads, keyboard/RTL or
reference-device tests. All eight locales have matching translated setup keys.
The user reports the earlier cache-only setup smoke passed on 2026-10-09.
The new lifecycle controls need the focused follow-up checklist in
[search-v2-smoke.md](search-v2-smoke.md#stage-7-preparation-controls--new-follow-up).
Downloading alone does not opt in or build the index; the separate controls below do that.

## Saved opt-in and index controls

`src/search-semantic/useSemanticSearch.ts` has one owner in `App`, independent of
Settings and reader navigation. `SemanticSearchSetup.tsx` renders functional
opt-in controls outside Developer mode. They share the existing cache format selector and
installer rather than adding another download path. The local WebView preference
`papercut.semantic-search.v1` stores only `enabled` and the fixed format enum;
invalid/unreadable values start off. It is device-local, not library metadata.
Failure to save refuses enabling; disabling still stops work and reports that a
previous preference may return after restart if persistence failed.

Enable loads only already cached, checksum-verified assets. Failure retains the
opt-in and presents an error/retry with lexical availability. Restart restores
cached activation only after saved consent, never a download or an index build.
Normal builds retain the preference without attempting unsupported activation.
**Build Index** explicitly synchronizes uploaded text; **Update Index** labels
the same command when incomplete coverage has indexed work. Compatible
checkpoints are reused. Use it after restart or library edits when coverage is
incomplete; already current coverage hides the action. Native invalidation already
hides stale documents.
Coverage reports compatible completed documents and ready passages, while job
counts report encoded/reused passages rather than an invented percent complete.
Wide settings pair **Passages Processed** with **Documents Indexed**; narrow
layouts stack them. The Library Index owns work/readiness status and its single
spinner. Status refreshes on library changes, settings open and busy polling;
**Retry Status** appears only after a failed read. Ready status requires the selected active provider, nonempty complete
coverage, no running build and no current error. Disabled coverage is zero for
ready passages/documents because no active model identity can be verified; this
does not imply deletion of checkpoint data.

Cancel or turn off to stop indexing cooperatively. Turn off unloads the provider
but retains model files and validated checkpoints; explicit index removal deletes
only derived data, while cache removal is separate. Closing Settings, opening a
reader does not cancel work or discard the controller.
Download terminal events carry a process-local `jobId`, selection, phase, byte
counts and nullable native `error`. The terminal outcome is emitted after native
reservation cleanup, including failure/unwind. The App retains the failed model
and diagnostic across Settings navigation with **Model Download Failed / Review**;
reviewing or rereading status does not erase it. A new download clears it and
stale/duplicate job events cannot replace the newer outcome. This is session
state, not persistent job history, and introduces no automatic retry/activation.

Semantic source/checkpoint errors include the document ID and, for section work,
the one-based section number. Document-wide failures omit the section. Context
adds no title/heading, passage or query text and introduces no logging. The exact
`Semantic indexing cancelled` outcome is preserved for the App's cancellation
handling. Tokenizer, inference and vector-validation failures leave committed
checkpoints reusable by a later explicit build; they do not publish partial
coverage or trigger an automatic rebuild.

UI actions reject conflicting/duplicate jobs, and generations ignore late
activation/index completions after disable. Status polling has one outstanding
poll per owner, only during pending work. Native cleanup can take time and does
not promise immediate RSS reclamation.

Frontend checks cover default-off/browser/capability boundaries, saved choice,
restart restoration, StrictMode effect replay, persistence failure, activation
failure/retry, late completion, explicit jobs, coverage, cancellation and slow
polling. A native mock-IPC regression proves provider disable signals the index
job without releasing its slot before cleanup. These are lifecycle checks, not
WebView/device, semantic ranking or memory acceptance.

## Native conceptual query

`document_uploads/semantic/query.rs` owns `search_semantic_query`. It is registered
in ordinary and explicit-provider builds. Ordinary builds fail recoverably;
provider builds require an already active verified cache. It never downloads,
activates, builds an index or changes All words/Broader. The isolated React hook
below calls it only while opted in.
Source/query text is never written to logs.

The camelCase `request` has `query`, optional `exactPhrases`, `documentUrls` and
`limit`. Absent/null scope means all uploads; an empty array means no uploads.
Scope accepts at most 10,000 URLs, each at most 2,048 bytes, without blank entries;
oversized requests fail rather than silently shortening selection. Result limit
is 1–50, default 50. Existing lexical query limits apply, and the active provider
also enforces its actual-token/character limit without truncation.

`availability` is `missing-index`, `incompatible-index`, `partial` or `ready`.
Disabled/not-compiled, corrupt data, busy query slot, changed sources/provider,
timeouts and native failures return errors for the enabled-path fallback.
Missing/incompatible indexes do not infer or initialize storage. Coverage counts
are for the scoped eligible/ready documents of a compatible index, before phrase
filtering; unavailable index responses do not establish scoped coverage. Partial
responses contain only completed compatible documents. Readiness is independent
of whether a query returns hits.

Scope and mandatory quoted phrases are verified before scoring or top-k selection.
Phrase verification reuses body-scoped FTS token/alias candidates followed by
lexical authored-source normalization at **document scope**; phrases may occur in different sections. A title-only occurrence does
not qualify. The displayed conceptual passage may be in another section; it is
not labeled as literal phrase evidence. Unquoted names/dates/words are conceptual
input, not required terms. The independent All words/Broader path keeps its own
semantics. There is no new synonym override list or rank fusion.

Rank documents by the cosine score of their best passage, descending. Source
order breaks passage ties and document ID breaks document ties. One document gets
one result regardless of overlapping/repeated chunks. Retain at most 50 candidates
and read one vector at a time. Returned results contain plain authored `text`,
`provenance: conceptual`, document identity/title/format, section ordinal/heading,
PDF page when present and section-local UTF-8 byte start/end. The displayed span
and exact embedded title/heading/body hash are verified against current source.
There are no invented highlights, term counts or occurrence maps. UTF-8 offsets
must not be used directly as JavaScript DOM offsets; React uses the existing
section/page/text reader-target boundary. Scores are similarity, not
confidence or proof that the passage answers the question.

The command opens the existing upload DB read-only. One read snapshot pins source,
index identity and vectors; a fresh metadata generation check rejects concurrent
source edit/deletion, removal or incompatible rebuild before returning. Provider
disable generations invalidate queued/scanning queries even after the same cache
is reloaded. One query slot bounds concurrent scans separately from indexing and
is released on errors/unwind. A cooperative ten-second budget includes queue time;
checks cannot interrupt a native call, a SQLite wait or one document's phrase
verification. Those waits may exceed the budget before the next check.

**Limit:** result memory is bounded, scan cost is linear in compatible scoped
passages, and phrase verification adds source reads. No ANN index, universal
latency or relevance threshold is claimed. Calibrate real library/device costs
before release; a timeout must leave lexical results available. Current tests
establish contracts and real cached inference, not a new overall quality win.

## Native scan scale check — 2026-10-09

Run the explicit synthetic scale probe without downloading models:

```sh
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --release \
  --lib document_uploads::semantic::store::tests::conceptual_query_scale \
  -- --ignored --exact --nocapture
```

It creates/removes its own database with 200 generic documents, 100,000 passages
and 384-dimensional unit vectors. Production retrieval checks result/scanned
counts, source scope, required/missing phrases, deterministic ties, encoding
count and read-only access. The printed first/warm/scoped/phrase timings exclude
inference and indexing; fixture creation leaves OS pages warm. Linux process RSS
high water includes fixture construction and all scans, not query-only or model
memory; it is absent on platforms without `/proc`. Keep this heavier probe
explicit rather than adding it to every ordinary test run.

The common-phrase probe exposed repeated FTS scans per document. Bound SQLite
`IN` subqueries now reuse each phrase's candidates; authored-source verification
and scoped readiness counting stay independent. A common phrase fell from
3,416.7 ms to 189.9 ms in the matched development-host probe; unrestricted
100,000-vector scans stayed about 155 ms. Narrow scope limits vector work, while
phrase candidate lookup still consults the library-wide FTS index. This does
not establish weak-device, cold-cache, model-inference or UI responsiveness.
See [current acceptance evidence](search-v2-progress.md) for packaging and gates.

## React conceptual results and fallback

When semantic search is enabled, SearchTab places secondary **Keyword (N) / Conceptual (N)** tabs above the results. App owns the selected view, initially Keyword, and retains it across session navigation/submissions. Each query still starts both channels independently; switching views neither searches again nor combines their rankings. Async completion never selects a different view. Query and document scope are shared. All Words/Broader radios and the accessible **Keyword matching** group are shown only for Keyword; Conceptual shows the required-quote helper instead.

Both views initially render ten returned document cards, with **Show More** revealing ten additional cards from the same response. The selector is sticky; panels stay mounted while hidden/inert so expanded evidence and each view's list window survive switching. The selected view and scroll place are distinct: switching from the scrolled selector restores that channel's remembered position, while a new request/scope resets windows and positions. No nested scrolling or retrieval pagination is added. Counts mean scoped **returned documents**, not literal occurrences, confidence, or an exhaustive total beyond the provider response limit. Pending and failed tab counts are omitted. More reports shown/returned counts and remains focusable but unavailable when the response is exhausted. Session view choice survives SearchTab unmount; card/window/scroll state is local to the mounted search view.

Keyword cards retain one list/count and a **Starter Document** badge rather than source-group sections. Existing uploaded-first and per-provider order remains; there is no cross-engine score fusion. Starter All Words candidates are filtered through each distinct unquoted term's Pagefind quoted search before scope/count/display limits; authored phrase verification is separate. No native retrieval changes. The conceptual summary retains **Conceptual Results** and returned-document count; matching explanations remain in Settings Technical Details. Healthy coverage remains in Settings; partial fallback retains **Documents ready for this search** coverage and withholds conceptual cards.

`src/search-semantic/useConceptualSearch.ts` owns invocation/state, App supplies submission/scope/library snapshots, and SearchTab owns result-view presentation. `ConceptualResults.tsx` renders authored passages; `SearchResults.tsx` retains lexical evidence, opening and failure behavior. The shared quote parser keeps unquoted text and authored clauses complete. All/empty/include/exclude scope is forwarded without widening selection. Scope changes wait for their matching submission rather than triggering another native scan.

Off, empty or invalid input has no query or fallback notice. Enabled unavailable/
preparing states keep lexical results usable; missing/incompatible/partial or
failed queries direct the user to the Keyword tab and Settings/retry recovery without automatically switching views. Partial
responses are deliberately withheld until the scope is complete. Known scoped
coverage is distinct from displayed-document count; a ready empty response is not
failure. The lexical channel can finish before the conceptual channel.

Query/submission, scope, source, model and index changes retire the old UI state.
Cleanup ignores late IPC completion after disable or unmount; it does not abort
an already running native scan. A queued microtask avoids double native dispatch
from StrictMode effect replay. Native busy/timeout errors require a later explicit
search retry rather than building an unbounded queue.

Conceptual passages render as plain React text, never marked HTML or invented
literal evidence. Similarity scores stay out of the UI. **Passage & Nearby Text**
initially shows the match and the nearest available source neighbour on each side,
in source order. **Show More Nearby Text (N)** reveals the remaining bounded
previews; **Show Less** restores that window. The match is emphasized and each
card owns its expansion, reset when its matched span changes. The toggle uses
`aria-expanded`/`aria-controls` and remains focusable through either transition.

Preview text is capped at 400 Unicode characters before an ellipsis, preferring a
whitespace boundary, with a six-line context clamp and 80ch reading width. Nearby
spans beginning within a section have a leading ellipsis. PDF display reflows
single line breaks and rejoins conservative ASCII/U+2010/U+2011 word wraps using
the existing shared predicate; blank-line paragraphs and inline punctuation stay
intact. Non-PDF authored line breaks remain. Only display strings are transformed;
source/index text, byte spans and reader targets remain original. More reveals
additional previews, not full text: use **Open Passage** to read its source.

**All Words** uses title case in English; **Broader** omits the library suffix,
with its imported-only scope retained in helper text across all eight locales.
Buttons use the existing
section/page/text reader-target boundary and the actual passage, not a query term
or a quote in a different section. Bytes are not DOM offsets. Existing legacy/
block/navigation limitations remain; real viewer targeting, keyboard/RTL, native
responsiveness and resource recovery need the focused rebuilt-app smoke. This
integration does not establish a new overall quality win or platform acceptance.

## Runnable checks

The command boundary now also runs through Tauri's existing test-only mock
runtime, using the actual command macros, JSON arguments and serialized responses.
The cache/path commands accept Tauri's standard runtime generic so the same
handlers compile for Wry and MockRuntime; their behavior is unchanged. Tauri's
`test` feature is a dev dependency only, absent from production feature trees.
No new crate version or test framework is introduced.

An ordinary Linux command check covers build capability, camelCase status,
invalid format arguments, disabled/cancel behavior and feature-off errors:

```sh
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
  --lib search_embeddings::ipc_tests::command_capability
```

The ignored real-cache command check requires an explicitly disposable Linux
data root and a read-only source of the already verified INT8 assets:

```sh
task_ipc_data=$(mktemp -d /tmp/papercut-cache-ipc.XXXXXX)
XDG_DATA_HOME="$task_ipc_data" PAPERCUT_EMBEDDING_IPC_TEST_DATA="$task_ipc_data" \
  PAPERCUT_EMBEDDING_TEST_MODELS="$CACHED_MODELS" \
  SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" LD_LIBRARY_PATH="$SHARED_LIB_DIR" \
  timeout 120s cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
  --features native-tts-shared,experimental-search-embeddings \
  --lib search_embeddings::ipc_tests::command_cache_lifecycle \
  -- --ignored --nocapture
```

This test refuses a normal data root, uses a separate test app identifier, and
copies rather than links/mutates the source assets. It checks read-only metadata,
missing/corrupt activation, size-presence versus integrity, cache reuse without
fetching, emitted progress, active-model removal protection, removal of other
formats, disable, fresh-state reuse without automatic activation and partial
staging cleanup. A held test reservation checks command cancellation/busy
semantics; the existing real-worker test separately verifies cleanup completion.
The test removes its own app-data tree. It never invokes a fresh-cache download.
MockRuntime does not start a browser; its profile path points to the existing
temporary directory so Tauri cannot create a real user profile.

The same ignored test also submits cached INT8 activation on a worker while
polling the real status command. It observes the loader reservation, rejects a
duplicate activation, disables one pending load, waits for its cancellation
result and verifies that a subsequent complete load succeeds. Status is polled
through that complete load, including native construction and warm-up. The
test uses two-second response assertions and a 30-second polling-loop guard;
the outer process timeout bounds a command that never returns. These are
generous regression guards, not product latency targets.

Three isolated runs on 2026-10-08 pass **1,018 status calls** collectively. The
maximum observed status response is **0.674 ms**; disable during the pending
load takes **0.094–0.187 ms**. Complete activations take **3.425–3.466 s**,
including verification and warm-up. Cancellation is intentionally early after
the reservation becomes visible, and finishes in about 21 ms; this does not
prove interruption inside an ONNX call. The separate cached-provider regression
covers discarded publication after completed native setup. All temporary app
data is removed and source model/runtime/executable hashes remain unchanged.
No production hook, lock change, dependency or model choice is added. Measurements
use a debug Linux build, cached assets and MockRuntime; they are not actual
WebView, release, weak-device or mobile responsiveness evidence.

This establishes command-handler integration on Linux, **not** actual WebView
IPC, upstream transfers, full-app startup recovery or mobile acceptance. Use
the [consolidated smoke checklist](search-v2-smoke.md) when interactive testing
resumes.

With the existing shared runtime available, ordinary provider regressions run:

```sh
SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" LD_LIBRARY_PATH="$SHARED_LIB_DIR" \
  cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
  --features native-tts-shared,experimental-search-embeddings --lib search_embeddings
```

For the production Arabic adapter, run the same shared-runtime command with
`--lib native_tts::engine::preprocess::tests`. It checks actual diacritization,
invalid tensors/models and concurrent calls against the app's initializer. The
optional `native_tts::engine::synth::tests::cached_arabic_preprocessing_smoke`
requires `PAPERCUT_ARABIC_TTS_MODEL_DIR` and `-- --ignored --exact --nocapture`;
it checks cached speech output without downloading. These app checks replace
current compatibility-crate guidance, not the historical four-order experiment
as evidence of a newly executed initialization-order matrix.

The real-model check is ignored by default and its default invocation never
downloads. Set
`PAPERCUT_EMBEDDING_TEST_MODELS` to a test-only directory containing the five
verified assets in each of `float32/` and `dynamic-int8/`, then explicitly run
`cached_provider_activation_switch_failure_and_unload -- --ignored --nocapture`
with the same Cargo options. Set optional test-only
`PAPERCUT_EMBEDDING_TEST_FORMAT` to `float32`, `dynamic-int8`, `base-float32` or
`large-float32` to isolate one selection for measurement; invalid values fail.
For Base/Large, supply the pinned assets under that selection's named folder;
Large needs the sixth external-weight file. Without this selector, only the
existing two Small formats run, keeping routine CI downloads unchanged.
This invocation reads only that
cache and uses test state plus disposable SQLite fixtures, without accessing the
installed library or app model directory.

For explicit runtime CI, set test-only `PAPERCUT_EMBEDDING_TEST_DOWNLOAD=1` as
well. The existing installer then provisions missing FP32/INT8 snapshots beneath
the supplied test root before activation, using pinned URLs, byte counts and
SHA-256 checks. A mock app handle receives progress; no installed application
data is accessed. Existing caches must pass verification and are never silently
repaired. CI uses `${{ runner.temp }}/search-provider-models`, with an OS/manifest
cache key. The exact test selector is:

```text
search_embeddings::provider::tests::cached_provider_activation_switch_failure_and_unload -- --ignored --exact --nocapture
```

Linux CI uses its already compiled debug library test with `LD_LIBRARY_PATH`
pointing at `tts/runtime/linux-x64-shared-libs`. macOS uses release tests and
`DYLD_LIBRARY_PATH` pointing at `tts/runtime/macos-shared-libs`; Windows uses
release tests with `src-tauri/target/release` prepended to `PATH` for native DLLs.
The Sherpa helper also stages the same DLL set into `release/deps`, next to the
test executable: Windows searches executable/system directories before `PATH`,
so the prefix alone cannot prevent an incompatible DLL from being selected.
These are prepared packaging runtime inputs, not relocated installer execution.
The regression also rejects identical vectors for distinct query/passage inputs;
successful vectors establish inference, not retrieval relevance or INT8
portability on platforms whose checks have not run yet.

For repeated native loads in one process, set test-only
`PAPERCUT_EMBEDDING_TEST_CYCLES=10` with the same command. The default remains one
cycle; accepted values are integers from 1 through 20. Each cycle activates and
disables both formats (or the requested single format), compares the English
query vector with the first load of that format, and repeats the existing
inference, failure cleanup and INT8 cancelled-publication checks. Phase logs
identify each cycle for external resource sampling. This exercises the actual
provider. Its first cycle per format also checks production indexing/retrieval on
disposable fixtures; later cycles do not repeat those fixtures. It does not test
WebView responsiveness or persistent app settings.

Three ordinary cache regressions cover integrity/staging/reuse/removal, blocked
concurrent operations and cancelled/panicked worker cleanup, and local HTTP status,
declared length, checksum, truncated body, idle timeout, HTTPS-only policy and
offline errors. They use tiny authored assets and a loopback server; they do not
download model weights or mutate the installed library. The real-model check also
verifies that removing an active model is refused before touching its cache.

## Native stability and memory follow-up

Unix Cargo builds previously copied shared libraries over existing destination
files. That can truncate a file already mapped by a running development app;
[`std::fs::copy` overwrites its destination](https://doc.rust-lang.org/std/fs/fn.copy.html).
The existing atomic-copy helper now stages with a process-specific name and
renames into place for both local archives and Unix runtime libraries. Failed
copies remove their staging file and preserve the previous destination. Windows
DLL copying is unchanged. This extends the existing sherpa build patch; it adds
no Libtashkeel fork or runtime dependency.

The retained stdlib-only regression verifies that an open old inode retains its
bytes, the destination receives new bytes and a failed copy preserves it:

```sh
rustc --edition=2021 --test src-tauri/vendor/sherpa-onnx-sys/runtime_copy.rs \
  -o /tmp/papercut-runtime-copy-test
/tmp/papercut-runtime-copy-test
```

On Linux, replacing the helper with the old direct copy makes this check fail.
A separate mapped-file control receives `SIGBUS` when the destination is held
truncated; the same reader survives atomic replacement. This demonstrates the
copy hazard, **not the cause of the earlier `SIGSEGV`**. That test executable
appears to have loaded the original runtime directory rather than the overwritten
Cargo profile copies. There is no crash backtrace establishing its cause.

After the fix, five isolated processes each pass the retained FP32/INT8 lifecycle
and real Arabic preprocessing/speech checks together, including cancelled native
setup and one mapped ORT image. These combined native checks peak at about
**1.52 GiB**, taking 14.52–14.79 seconds per complete check on this host. No native
crash recurs; this bounded repetition does not close release stability.

Separate single-format native-test processes, with RSS sampled through Linux
`/proc`, show:

| Format | Whole-check high water | RSS after activation/warm-up | RSS immediately after disable |
| --- | --- | --- | --- |
| FP32 | 1.42 GiB | 997 MiB | 613 MiB |
| INT8 | 846 MiB | 663 MiB | 551 MiB |

The INT8 whole check includes a second sequential session discarded after
cancellation. Activation-to-ready takes approximately 8.5 seconds FP32 and
3.3 seconds INT8 in these individual runs, including verification and warm-up.
These are native test processes with existing filesystem caches, not cold-disk,
full-app, steady query latency, long-lived reload-cycle or mobile measurements.
The prior five combined processes include TTS; their costs are not directly
comparable to an embedding-only process.

In a separate FP32 debugger control, calling glibc `malloc_trim(0)` after the
provider was removed reduced process RSS from 614 MiB to 50 MiB, and the test
finished normally. This demonstrates substantial allocator retention in that
run; it does not identify every allocation or prove that all platforms behave
the same way. No allocator override, trimming call, dependency fork or weaker
model default is added. Disabling removes the session, not a guaranteed quantity
of process RSS. Keep memory-pressure and repeated-load responsiveness as device
acceptance checks.

### Same-process reload check — 2026-10-08

The extended cached-provider check passes two independent processes with ten
cycles per format each: **40 successful activations and 20 cancelled native
loads**, in addition to the unchanged one-cycle control. Each process keeps one
provider state through its repeated loads; each format's English vector equals
its first-cycle vector. Existing Arabic/input validation, disable-first
switching, failed-load cleanup, cancellation and single-ORT assertions repeat.
Zero, over-limit and non-integer cycle counts fail before requesting any assets.
Seven ordinary provider/cache/command tests pass; two asset-dependent tests stay
ignored in that ordinary run. The cached provider test is explicitly exercised
separately. No crash recurs.

| Measurement | Ten-cycle process A | Ten-cycle process B |
| --- | --- | --- |
| Complete check | 151.49 s | 148.93 s |
| Whole-process high water | 1.803 GiB | 1.803 GiB |
| Median FP32 activation | 8.41 s | 8.29 s |
| Median INT8 activation | 3.20 s | 3.16 s |

The default one-cycle control peaks at 1.434 GiB. After its first FP32/INT8
cycle, each repeated process retains about 626 MiB. From cycle two, completion
RSS stays near 665 MiB, ending near 655–657 MiB after the final cancelled load.
The higher reload peak matters for headroom even when post-disable RSS plateaus.
This bounded observation does not establish a leak-free process or justify an
allocator hook. The earlier crash remains unexplained.

These are debug Linux development-host checks with warm filesystem caches.
The parent samples `/proc/<pid>/status` every 50 ms and at received phase logs;
phase RSS is approximate, not synchronized. Neither child reports swapped pages
in the samples, but the host already has substantial swap use/background memory
pressure. Activation includes asset verification, construction and warm-up.
There is no TTS synthesis, real WebView, release build, foreground responsiveness,
device-pressure or mobile acceptance in this probe. Model files and installed
app data are not modified. Interactive smoke remains deferred.

## Process-interruption recovery — 2026-10-08

An ordinary Linux regression now runs the shared installer in disposable child
processes with two tiny authored assets. The parent waits for an atomic checkpoint
marker, sends `SIGKILL` to its own child and confirms signal 9, then starts a
fresh recovery process. Children are reaped on assertion failures and waits have
ten-second guards. No production recovery hook or installer change is added.

| Interrupted checkpoint | Explicit retry in a fresh process |
| --- | --- |
| Partial second asset | No final cache; abandoned staging discarded and both assets fetched again. |
| Verified staging before rename | No final cache; staging discarded and both assets fetched again. |
| Immediately after publish rename | Published cache verified and reused with zero fetches. |

The recovered state is inactive with no download reservation, verified bytes
remain intact, staging is absent and ordinary removal succeeds. Three targeted
parent runs cover nine abrupt kills and nine fresh recovery processes; the full
ordinary provider suite also passes **eight tests**, with two asset-dependent
checks ignored. Run this regression using the existing provider test command
with the filter
`search_embeddings::cache::tests::process_interruption_recovers_staging_and_reuses_published_cache`.
It requires neither model weights nor network access.

This covers process termination at the installer/publish boundary on Linux,
not power loss, filesystem durability after reboot, full-sized model transfers,
actual WebView startup or mobile recovery. Explicit retry restarts abandoned
transfers; it does not resume their byte offsets. The next section adds
command-level recovery with real cached assets. Smoke remains deferred and the original
crash, resource and platform acceptance gates remain open.

## Real-cache command restart — 2026-10-08

The existing ignored command check now starts a child with the same disposable
app identifier and real pinned INT8 cache. The child verifies the cache through
`search_embedding_download`, explicitly activates it through
`search_embedding_configure` and publishes an atomic readiness marker only
after successful native warm-up. The parent confirms `SIGKILL` termination,
then starts a second child against the surviving cache. Fresh status has null
active/loading/downloading/encoder fields; cached weights never auto-activate.

That recovery child uses the public handlers to verify/reuse cached assets,
activate with the expected encoder identity, reject removal while active,
disable and remove. The parent then confirms cache absence. Owned children are
reaped on failures; readiness/recovery waits have 30-second guards and the
existing outer process timeout remains required. No production hooks or new
recovery algorithm are added.

Three isolated parent runs pass **three native SIGKILL/restart cases** along
with the existing integrity, cancellation, status and removal checks. Complete
checks take 16.58–17.69 seconds, including local cache preparation and all three
processes; these are not restart-only or query timings. Eight ordinary provider
regressions pass with two asset-dependent checks ignored. Copied app data is
cleaned up; source assets, runtime, test source and executable hashes remain
unchanged. No upstream model transfer or installed-library mutation occurs.

This is debug Linux/MockRuntime/cached-INT8 evidence. It does not prove real
WebView startup, interrupted upstream transfers, power-loss durability, release
profile stability or mobile behavior. The original native crash and retained
memory caveats remain open. Smoke is still explicitly deferred. Next validate
the existing native checks under the optimized release profile.

## Optimized native validation — 2026-10-08

The same app tests now pass under Cargo's optimized release profile with
`native-tts-shared,experimental-search-embeddings`, compiled locked and offline.
The existing provider-test command works with `--release`; no separate test
implementation, production hook or dependency change is needed. Eight ordinary
provider/cache/IPC regressions pass, two cached checks ignored by that invocation.
The existing loopback HTTP regression requires socket access; no upstream model
download is performed.

| Explicit cached check | Release result |
| --- | --- |
| Default provider cycle | FP32/INT8 inference, selection, failure cleanup and cancellation pass; 2.57 s complete, 1.424 GiB whole-process high water. |
| Ten provider cycles per format | Reload vector equality and EN/AR controls pass; 22.57 s complete, 1.791 GiB high water; about 653 MiB retained after the final cycle. |
| Combined provider and Arabic speech | Both tests pass in one process; 2.52 s complete, 1.524 GiB high water; expected diacritization and finite non-silent audio. |
| Three isolated real-cache command runs | Three native kill/restart recoveries pass; 2.66–3.18 s per complete check, including local cache copying and children. |

In the repeated run, median activation is 0.83 s FP32 / 0.58 s INT8, including
verification, construction and warm-up. Across the command runs, 279 status calls
complete with a maximum observed 0.154 ms response; early cancellation commands
take 0.043–0.293 ms. Cancellation is requested before native construction, not
proven interruptible inside ORT. The separate provider check discards a completed
native load when cancellation prevents publication.

The parent samples provider `/proc` status every 50 ms (10 ms for the combined
check) and on received phase messages. Measurements are approximate, with warm
filesystem caches and host background memory pressure; provider samples report
no child swap. These are not matched debug/release benchmarks, query timings,
foreground responsiveness or promises about weak hardware. Disable still does
not guarantee immediate RSS recovery. The combined check uses the threaded test
harness, not app-level reader/audio scheduling or sustained concurrent workloads.

Model-cache copies are disposable and removed; pinned source assets, native
runtime, executable and test-source bindings are verified. Exact build/test logs,
sources, resource samples and executable are retained in private local evidence.
No installed library is mutated. This establishes bounded optimized Linux test
coverage, not WebView/mobile acceptance, interrupted upstream transfers,
power-loss durability or the cause of the earlier native crash. Interactive
smoke remains deferred and representative quality still holds Stage 7 closed.

Next review release acceptance for the model-free tranche separately from the
embedding experiment. Further benchmark matrices or model/fusion sweeps are not
justified by these passing checks; remaining gates need different evidence.

## Remaining gates

Native stability and device resource costs remain release gates. Functional
Settings, compatible incremental indexing and separate conceptual results are
implemented with a default-off toggle; enabled unavailable/incomplete/failed
search visibly falls back to lexical. The explicit provider build is still
required. Automated cached-handler/scale checks and normal packaging do not
establish WebView reader targets, RTL, responsiveness or mobile acceptance.
The earlier crash remains unexplained and disable does not promise immediate
RSS recovery. Representative independent quality and all advertised platforms
remain open. Interactive lifecycle/search/EPUB/audio checks remain in the user's
deferred consolidated smoke pass; this deferral is not acceptance. Expand model
choices only after measured quality/cost evidence.
