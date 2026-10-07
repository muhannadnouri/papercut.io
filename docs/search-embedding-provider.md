# Experimental search embedding provider

This is an app-level, default-off **provider prototype**, not production hybrid
search. Ordinary search never invokes it. There is no settings UI, automatic
model download, semantic index or ranking integration in this pass. Activation
is explicit and session-only; restarting returns to disabled.

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
  requested API. No runtime download or replacement occurs during activation. Initialization
failures are cached; repairing a packaged runtime requires a restart.

`compiled` means that the provider is present in the build, not that a platform
has passed release validation. Status also returns `platform`, `activeFormat`,
`loadingFormat` and `encoderKey`. A non-null active format means integrity checks,
model construction and a finite normalized 384-dimensional warm-up vector passed.
It does not establish relevance quality or resource suitability.

## Explicit cached-model activation

The native `search_embedding_configure` command accepts `format: null` to
cancel/unload, or `"float32"` / `"dynamic-int8"` to activate an already staged
snapshot. It accepts no arbitrary path, model URL or unverified model identifier.
All asset reading and native loading/cleanup run on the blocking pool.

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
not the shared ORT environment used by other consumers. Native cleanup serializes
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

## Runnable checks

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

## Remaining gates

Investigate retained native stability/resource costs before exposing model controls
for release. The next bounded pass can then add verified cancellable
staging/download controls and settings UX around this provider. It must include disk-space checks, partial-file
cleanup, offline errors, explicit opt-in and removal semantics. Reuse existing
job/download patterns where they fit; do not repurpose the voice catalogue.
Before enabling hybrid results, representative independent quality, compatible
incremental indexes, lexical fallback, device responsiveness/resource limits and
all advertised platforms must pass. Model choices can expand after measured
quality/cost evidence. Interactive search/EPUB/audio and later model controls stay
in the user's consolidated final smoke pass; this deferral is not acceptance.
