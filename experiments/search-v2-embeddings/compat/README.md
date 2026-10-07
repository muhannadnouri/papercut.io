# Libtashkeel / ORT compatibility

Papercut uses published `libtashkeel_base` 1.5.0 with its built-in ORT feature
disabled and a caller-supplied ORT `2.0.0-rc.13` engine. The app has no FastEmbed
dependency or local Libtashkeel override. Its previous vendored adaptation was
removed; archived results are historical evidence.

## Official interface route

The check in [`same_process.rs`](same_process.rs) reuses the app's small
[`InferenceEngine` adapter](../../../src-tauri/src/native_tts/engine/tashkeel.rs),
with the **published package's** `ort` and default
features disabled. Libtashkeel's normalization, maps, segmentation and
diacritization stay upstream. Papercut owns the small session/tensor adapter;
this avoids maintaining a package fork, not all integration code.

The nested manifest pins official Libtashkeel 1.5.0, FastEmbed 7.1.0 and ORT
`rc.13`, with the existing sherpa 1.13.4 build patch. It does not modify the
retrieval experiment's dependency graph. The separately
built **check** runs all three engines in one process; it does not propose a
separate production worker. Native binaries and models must already exist.

## Run the Linux check

Set `SHARED_LIB_DIR` to the cached sherpa shared-library directory and
`CHECK_TARGET_DIR` to an external writable build directory. Set `TASHKEEL_ONNX`,
`E5_FP32_DIR`, `E5_INT8_DIR` and `ARABIC_TTS_DIR` to the corresponding existing
assets. Each E5 directory needs `onnx/model.onnx`, `tokenizer.json`, `config.json`,
`special_tokens_map.json` and `tokenizer_config.json`. No download feature is
enabled. The check reads `/proc/self/maps`, so it is Linux-specific.

```sh
SHERPA_ONNX_LIB_DIR="$SHARED_LIB_DIR" CARGO_TARGET_DIR="$CHECK_TARGET_DIR" \
  cargo build --offline --locked --manifest-path experiments/search-v2-embeddings/compat/Cargo.toml

LD_LIBRARY_PATH="$SHARED_LIB_DIR" ORT_DYLIB_PATH="$SHARED_LIB_DIR/libonnxruntime.so" \
  "$CHECK_TARGET_DIR/debug/search-v2-runtime-compat" \
  speech-first none "$TASHKEEL_ONNX" "$E5_FP32_DIR" "$ARABIC_TTS_DIR"
```

Repeat with `embedding-first`, then both orders with `dynamic-int8` and
`E5_INT8_DIR`. The policy is explicit; directory names do not select it.
The check asserts two known Arabic outputs, repeats six generic Arabic inputs,
exercises four concurrent preprocessing calls, rejects inconsistent input
lengths and invalid ONNX, validates three finite normalized 384-dimensional
vectors, requires unchanged vectors after speech, checks finite non-silent
audio, and verifies one native ORT image at the requested path. JSON on stdout
supports comparison across orders and with an official-backend baseline.

## Verified result and remaining work

On 2026-10-07, all four combinations pass using the cached Linux ORT 1.27.0.
Six Arabic outputs also match a freshly run official `rc.1` backend exactly;
vectors match across initialization orders within each format. Five separate
invalid-order/policy, missing/corrupt-model and missing-tokenizer runs fail with
recoverable errors and no success report. These are compatibility assertions,
not audio listening grades, relevance evidence or performance benchmarks.

The unchanged upstream model is **4,788,213 bytes**, SHA-256
`52c40d42ca094ff9ff07a091d0a1eafd7baa5cf2388e60a9cc89bacd8e5e955f`.
Its bytes and the upstream Rust files were compared with the cached published
crate archive, whose checksum matches this lockfile. The built-in backend's
model bytes are private; the app now [provisions that unchanged asset explicitly](../../../src-tauri/tts/libtashkeel-1.5.0/README.md),
with a build-time checksum, compile-time embedding and packaged license. Set
`TASHKEEL_ONNX` to `src-tauri/tts/libtashkeel-1.5.0/model.onnx` for this check.
No upstream Rust source is copied. Do not locate a user's Cargo cache at runtime
or silently fetch mutable upstream model files.

A private copy of the full app manifest also resolves the combined Linux
feature graph with one ORT/ort-sys `rc.13` pair and official Libtashkeel with no
features. It requires removing the old direct `ort-sys/load-dynamic` feature
and using `ort/load-dynamic`; resolution is **not a full app build**.

The app now adopts the same adapter; Linux native and package validation are
recorded in [the progress report](../../../docs/search-v2-progress.md).
Other desktop targets and mobile remain unverified. A Linux check of the linked
initializer does not establish an iOS static-archive/device build.
Stage 7 opt-in indexing/model controls still need quality and resource evidence;
runtime compatibility alone is not a retrieval win. Interactive smoke is
deferred by the user to one consolidated final pass.
