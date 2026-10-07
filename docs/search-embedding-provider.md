# Experimental search embedding provider

This is an app-level, default-off **provider prototype**, not production hybrid
search. Ordinary search never invokes it. Developer-mode settings offer explicit
model-cache preparation; there is no automatic model download, semantic index or
ranking integration. Activation is explicit and session-only; restarting returns
to disabled.

Desktop-first validation is sequencing, not a desktop-only product restriction.
The commands and state are shared Rust code with no desktop-only availability
check. Android and iOS still require their own build, linking, lifecycle and
resource tests before availability can be advertised.

## Build and capability boundary

- Normal builds omit FastEmbed entirely. `search_embedding_status` reports
  `compiled: false`; activation fails recoverably and disabling remains valid.
- `experimental-search-embeddings` adds the provider and dynamic ORT loading,
  suitable for checking existing shared-runtime desktop/Android configurations.
- `experimental-search-embeddings-linked` adds the provider without dynamic
  loading, for the linked runtime configuration used by iOS. Choose this rather
  than the dynamic feature for an iOS build. A linked initializer test on Linux
  is not an iOS static-archive or device test.
- FastEmbed 7.1.0 enables ORT API 24 only in provider builds. Ordinary TTS retains
  API 17. Both consumers use one shared initializer and ORT `rc.13`; official
  Libtashkeel remains unchanged. The existing packaged runtime must support the
  requested API. No runtime download or replacement occurs during activation.
  Initialization failures are cached; repairing a packaged runtime requires a restart.

`compiled` means that the provider is present in the build, not that a platform
has passed release validation. Status also returns `platform`, `activeFormat`,
`loadingFormat`, `downloadingFormat` and `encoderKey`. A non-null active format means integrity checks,
model construction and a finite normalized 384-dimensional warm-up vector passed.
It does not establish relevance quality or resource suitability.

## Explicit cached-model activation

The native `search_embedding_configure` command accepts `format: null` to
cancel/unload, or `"float32"` / `"dynamic-int8"` to activate an already staged
snapshot. It accepts no arbitrary path, model URL or unverified model identifier.
All asset reading and native loading/cleanup run on the blocking pool. Disabling
also cancels a pending model download; its busy slot remains occupied until the
worker stops and staging cleanup finishes.

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

These are two formats of the existing experimental baseline, not a product model
catalogue or a decision to impose the smallest encoder on every device. The
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
`encoderKey` includes snapshot, exact ONNX checksum, format, pooling, prefix, dimensions, token limit
and per-text policy. A future index must additionally include source content,
extraction, normalization and chunking versions; this key alone is not an index
compatibility contract.

## Explicit downloads and cache removal

The feature-gated native commands provide the backend for opt-in prototype
settings. There is no automatic download. Downloading never activates the
provider, persists an opt-in preference or changes ordinary search.

| Command | Contract |
| --- | --- |
| `search_embedding_cache_status` | Returns the two fixed formats, `expectedBytes` and `filesPresent`. Presence checks exact file sizes and regular files, not fresh checksums. Activation always verifies actual bytes again. |
| `search_embedding_download` | Accepts only the existing `format` enum. Downloads the five checksum-pinned assets from the fixed upstream revision, verifies staging and promotes the complete directory. A verified existing cache is reused without an HTTP request. |
| `search_embedding_cancel_download` | Requests cancellation without unloading another active provider. Status remains busy until the worker returns. |
| `search_embedding_remove` | Removes only the selected model-cache directory and its interrupted staging directory. Removal is idempotent; pending activation/download must finish first, and an active selected model must be disabled first. There is no app semantic index to remove yet. |

No command accepts a URL, filesystem path or arbitrary model identifier. The
download and loader share one asset manifest and the same bounded streaming
size/SHA-256 checker. The two existing encoder identities are unchanged. Production
requests are HTTPS-only, including redirects. The upstream INT8 path remains the
AVX512/VNNI-named export; downloading it does not establish portability.

Before writing, the installer reuses library transfer's free-space preflight:
the full payload plus **64 MiB** of reserve on the same filesystem. Files are
written beneath a sibling `float32.installing` or `dynamic-int8.installing`
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
events; app IPC/live upstream-download smoke remains deferred.

## Developer-mode model setup

Open **Settings → Developer mode** in a Tauri build. The model-setup section
reports unavailability in normal builds, which still omit FastEmbed. Explicit
provider builds offer the two existing formats; there is no desktop-only UI
check or assertion of Android/iOS acceptance. The panel defaults to FP32, shows
installed-file size plus the 64 MiB disk reserve, measured native-test memory
costs and the INT8 export's unverified CPU/mobile compatibility.

An unchecked consent box names Hugging Face and this device as the destination.
Each format change and completed attempt clears consent. Only an explicit
**Download model files** or **Verify cache** click invokes the native installer.
The latter verifies a valid existing cache without fetching; metadata-only
presence is not labeled as verified. **Remove model files** is available even
for an incomplete/missing cache, so it can clear interrupted staging. Active
selected caches cannot be removed, and pending native activation/download blocks
cache mutation. No activation, preference persistence or search action is
introduced by this UI.

The panel subscribes before reading status, polls only during pending work and
recovers native busy/cache state when reopened. Closing settings, changing tabs
or hiding Developer mode detaches listeners and timers; it does not cancel the
native download. Cancellation remains busy until the worker finishes, and is
separate from unloading an active provider. Errors are shown with retry and
explicit damaged-cache removal guidance. An observed failure/cancellation event
also supplies a terminal message when the panel did not start the job. There is
no persistent job/error history: reopening after a finished job shows current
cache presence, and verification remains an explicit action.

Five frontend regressions use the repository's hook-mocking pattern and
server-rendered controls to exercise default-build availability, explicit
consent/format resets, duplicate-click rejection, pending cancellation,
native-state recovery after reopening, unrelated progress, active-cache removal
refusal, successful verification, failure/retry and delayed listener cleanup:

```sh
npm test -- src/components/AppSettings/SearchModelSetup.test.tsx
```

These checks do not replace WebView/app IPC, actual downloads, keyboard/RTL or
reference-device tests. All eight locales have matching translated setup keys.
Interactive setup checks join the user's deferred consolidated search/EPUB/audio
smoke pass. Setup remains preparation for a prototype, not conceptual search.

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
  cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
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

This establishes command-handler integration on Linux, **not** actual WebView
IPC, upstream transfers, process-interruption recovery or mobile acceptance. Use
the [consolidated smoke checklist](search-v2-smoke.md) when interactive testing
resumes.

With the existing shared runtime available, ordinary provider regressions run:

```sh
SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" LD_LIBRARY_PATH="$SHARED_LIB_DIR" \
  cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
  --features native-tts-shared,experimental-search-embeddings --lib search_embeddings
```

The real-model check is ignored by default and never downloads. Set
`PAPERCUT_EMBEDDING_TEST_MODELS` to a test-only directory containing the five
verified assets in each of `float32/` and `dynamic-int8/`, then explicitly run
`cached_provider_activation_switch_failure_and_unload -- --ignored --nocapture`
with the same Cargo options. Set optional test-only
`PAPERCUT_EMBEDDING_TEST_FORMAT` to `float32` or `dynamic-int8` to isolate one
format for measurement; invalid values fail. This test reads only that cache and uses in-memory
state, not the installed library or app model directory.

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

## Remaining gates

Native stability and device resource costs remain release gates. Model-setup
settings and mock-runtime command validation are implemented; the consolidated
smoke packet is prepared, not accepted. Setup must not promise conceptual results
while search still does not use the provider. The next work is actual acceptance
evidence rather than another speculative feature or a voice-catalogue repurpose.
Before enabling hybrid results, representative independent quality, compatible
incremental indexes, lexical fallback, device responsiveness/resource limits and
all advertised platforms must pass. Model choices can expand after measured
quality/cost evidence. Interactive search/EPUB/audio and later model controls stay
in the user's consolidated final smoke pass; this deferral is not acceptance.
