# SILMA TTS Python Sidecar Development Guide

Last updated: 2026-07-13

This is the working guide for bringing SILMA TTS into Papercut as a desktop-only
Python sidecar while keeping the existing sherpa-onnx audiobook path.

The goal is not to build a second audiobook system. The goal is to add one more
offline synthesis backend behind the current save/resume/export/playback
pipeline.

## Decision

Use a bundled desktop sidecar for SILMA first.

- Keep sherpa-onnx as the native cross-platform backend for Kokoro, Piper, and
  Supertonic.
- Add SILMA as a desktop-only backend powered by the official Python runtime.
- Download SILMA model files on demand, pinned by revision and verified by hash.
- Do not bundle SILMA weights in app installers.
- Do not support Android or iOS sidecar execution.
- Do not introduce a general plugin system, remote service mode, or arbitrary
  user Python environment.

Ponytail rule for this project: prove one good WAV from the sidecar before
abstracting anything broad.

## Why Sidecar

SILMA is not a sherpa-onnx model family. It is based on F5-TTS and is distributed
as a Python/PyTorch runtime. A direct sherpa catalog entry is not realistic
unless sherpa-onnx adds F5/SILMA support upstream or we write a native F5
runtime ourselves.

The sidecar path buys us:

- access to the official SILMA inference behavior first;
- no Rust reimplementation of F5-TTS, CATT, NeMo, or PyTorch model loading;
- reuse of Papercut's audiobook pipeline;
- a clean desktop-only feature gate while mobile stays sherpa-only.

It costs us:

- larger desktop packages;
- Python/PyTorch packaging and signing work;
- a second process lifecycle;
- desktop-only support;
- more security and product policy around voice cloning.

## Current Papercut TTS Shape

The existing native TTS path is already close to what we need.

- React owns model selection, voice selection, document chunking, save UI,
  playback state, highlighting, and saved-audiobook records.
- Rust owns model install/status, chunk WAV generation, cache scanning,
  manifests, import/export, and mobile playback-track preparation.
- The save loop in `src-tauri/src/native_tts/engine/save.rs` already reduces the
  engine requirement to: for each missing chunk, produce a valid WAV at a known
  path.
- The current direct sherpa coupling lives mainly in:
  - `src-tauri/src/native_tts/state.rs`
  - `src-tauri/src/native_tts/engine/models.rs`
  - `src-tauri/src/native_tts/engine/model.rs`
  - `src-tauri/src/native_tts/engine/synth.rs`
  - `src-tauri/src/native_tts/engine/paths.rs`

Reusable as-is:

- document loading and readable text extraction;
- deterministic audiobook IDs;
- chunk source signatures;
- source spans and highlighting;
- save/resume/cancel progress;
- chunk cache and manifest format, with model/preprocessor metadata;
- bundle export/import and stitched WAV export;
- saved-only playback;
- diagnostics concepts.

Needs backend work:

- loaded engine state currently stores `SherpaTtsEngine` directly;
- model install path now derives its backend directory from catalog metadata;
- model archive extraction assumes sherpa-style tar.bz2 layouts;
- `VoiceDefinition` assumes numeric speaker IDs;
- silent placeholder writing uses sherpa's WAV writer;
- diagnostics hard-code `sherpa-onnx` labels in a few places.

## Target Architecture

```text
React audiobook UI
  -> src/tts/api/nativeTts.ts
    -> Tauri native_tts commands
      -> save/resume loop
        -> LoadedTtsEngine
          -> Sherpa backend
             -> sherpa_onnx::OfflineTts
             -> writes chunk WAV
          -> SILMA backend, desktop only
             -> supervised Python sidecar
             -> sidecar loads model once
             -> sidecar writes chunk WAV
      -> existing manifest/cache/export/playback code
```

The Rust save loop should not know Python details. It should know only:

- selected model metadata;
- selected text preprocessor metadata;
- selected voice metadata;
- loaded backend label;
- synthesize this text to this output WAV path.

## Backend Boundary

Add the smallest useful boundary after the first sidecar spike passes.

Likely shape:

```rust
enum LoadedTtsEngine {
    Sherpa(SherpaTtsEngine),
    #[cfg(all(desktop, feature = "native-tts-silma-sidecar"))]
    Silma(SilmaTtsEngine),
}
```

Methods:

- `model(&self) -> &'static ModelDefinition`
- `backend_label(&self) -> String`
- `synthesize_to_file(text, voice, speed, output_path) -> FileSynthesisResult`
- `sample_rate()` only if shared silent WAV writing needs it

Avoid a trait object until there are at least two real implementations in-tree.
An enum is easier to read, easier to compile-gate, and enough for this feature.

## SILMA Worker Contract

Use a persistent worker process. Do not spawn Python per chunk.

Preferred initial protocol: JSON Lines over stdin/stdout.

Reasons:

- no port selection;
- no localhost firewall prompts;
- no HTTP server dependency;
- easy Rust supervision;
- easy log separation through stderr.

Request examples:

```json
{"id":"1","op":"health"}
{"id":"2","op":"load_model","model_dir":"/app-data/models/silma-tts/v1"}
{"id":"3","op":"synthesize","text":"...", "voice":"silma-ar-default", "speed":1.0, "seed":1234, "output_wav":"/app-data/audiobooks/.../chunks/00001.wav.tmp"}
{"id":"4","op":"shutdown"}
```

Response examples:

```json
{"id":"1","ok":true,"version":"0.1.0"}
{"id":"2","ok":true,"sample_rate":24000}
{"id":"3","ok":true,"sample_rate":24000,"audio_duration_sec":7.42,"wav_bytes":356812,"synthesis_ms":18340}
{"id":"3","ok":false,"error":"..."}
```

Worker rules:

- stdout is protocol only;
- stderr is logs only;
- every response includes the request id;
- worker writes the WAV itself to the requested temp path;
- Rust validates the WAV before committing it;
- sidecar never receives arbitrary shell args from the frontend;
- sidecar never writes outside app-owned model/cache/temp paths.

If JSONL becomes painful, switch to local HTTP later. Do not start there.

## Voice Model

SILMA is voice-cloning style TTS, so a Papercut "voice" is not a numeric speaker
ID. It is a voice profile:

- stable voice id;
- display name;
- reference audio path;
- reference text;
- optional language;
- license/consent metadata;
- default seed policy.

For the proof of concept, use SILMA's sample reference only if its license and
redistribution terms are acceptable for development use.

For production, use owned or explicitly licensed reference audio. Do not ship a
"clone any voice" UI in the first release. That is a product/legal feature, not
just an engineering switch.

## Model Download

SILMA model files should follow the existing Papercut model install pattern:

- download on demand;
- write through a temporary file;
- verify SHA-256 before install;
- validate required files;
- atomically promote into app data;
- report progress to the same model-install UI.

SILMA-specific additions:

- pin a Hugging Face revision, not just a moving branch;
- verify each required file or verify one archive we control;
- expect large downloads;
- resume interrupted/range downloads for large model files;
- store under `models/silma-tts/<model-directory>/`, using the same backend
  directory hook that keeps existing sherpa models under `models/sherpa-onnx`.

Required files are expected to include at least:

- `model.pt`
- `vocab.txt`

The current installer downloads only those two files because the official worker
path resolves them directly. Add `config.yaml` or other F5 runtime files here
only after the packaged sidecar proves it needs them.

Current pinned SILMA model revision:

```text
d2515317033803648ecb8844765db9e583afecf9
```

If downloading individual Hugging Face files is fragile, create a pinned release
archive in Papercut-controlled release storage that contains only upstream SILMA
files plus a manifest. Keep licenses and source URLs in that archive manifest.

## Text Preprocessing

Do not run Piper's Libtashkeel path for SILMA.

SILMA advertises its own Arabic handling through CATT and NeMo text processing.
Expose this as SILMA model capability data instead:

- `silma-default`: use SILMA's normal text processing;
- maybe `none`: bypass optional SILMA preprocessing only if the worker supports
  this cleanly and it is useful for debugging.

The source chunk text must remain unchanged. Only synthesis text may be changed.
Highlighting continues to align to original document text.

## WAV Requirements

Every generated chunk must be:

- a valid WAV file;
- mono PCM unless the playback/export code is expanded;
- one stable sample rate per audiobook;
- nonzero duration;
- committed atomically after validation.

The worker may write WAV output, but Rust remains the final authority:

- write to temp path;
- validate with existing `wav_info`;
- commit with `commit_staged_file`;
- update manifest from WAV headers.

`wav_sink.rs` now owns temp-file creation, WAV validation, atomic commit, timing
metadata, and silent-placeholder writing. Sherpa and SILMA both write through
this shared sink path; SILMA asks the sidecar to write to the temp path before
Rust validates and commits it.

## Tauri Packaging Plan

Tauri supports desktop sidecars through bundled external binaries. The current
app does not yet initialize `tauri-plugin-shell` and does not configure
`bundle.externalBin`.

Implementation tasks:

- add `tauri-plugin-shell` to `src-tauri/Cargo.toml`;
- initialize `tauri_plugin_shell::init()` in `src-tauri/src/lib.rs`;
- add `bundle.externalBin` or a resource-based worker layout for desktop builds;
- add platform-specific worker prep scripts;
- do not expose generic shell permissions to React unless React directly starts
  the worker, which it should not need to do.

### Onefile vs Onedir

Prototype path:

- PyInstaller `onedir` sidecar;
- easiest way to avoid extracting a multi-GB Torch payload into `/tmp`;
- compatible with a direct executable env override while bundle layout is still
  under test.

Rejected first spike:

- PyInstaller `onefile`;
- produced a 3.18 GB Linux executable;
- failed packaged `--self-test` while extracting `torch/lib/libtorch_cpu.so`,
  even with `TMPDIR` pointed outside `/tmp`;
- keep only as a diagnostic option, not the release direction.

Production path:

- PyInstaller `onedir`;
- faster startup;
- fewer temp extraction problems;
- easier to inspect and sign dependencies;
- requires validating Tauri bundle layout so the executable can find its
  `_internal` dependencies.

Do not assume `externalBin` alone copies PyInstaller's whole `onedir`. If needed,
bundle the sidecar folder as a Tauri resource and spawn the executable from the
resolved resource path.

Linux packaging spike status:

- `PAPERCUT_BUNDLE_SILMA_TTS=1 npm run desktop` stages the PyInstaller onedir
  worker into `src-tauri/tts/runtime/silma-sidecar-linux-x64/` before Tauri
  scans bundle resources.
- `src-tauri/tauri.linux.conf.json` bundles that staged directory into the app
  resource path `silma-sidecar/`.
- Rust worker launch now checks, in order:
  - `PAPERCUT_SILMA_WORKER_BIN`;
  - explicit `PAPERCUT_SILMA_WORKER` or `PAPERCUT_SILMA_PYTHON`;
  - bundled Linux resource executable;
  - editable repo worker script.
- Normal desktop builds keep an empty resource directory and do not build or
  copy the large sidecar unless `PAPERCUT_BUNDLE_SILMA_TTS=1` is set.

Linux packaged-sidecar validation command:

```bash
PAPERCUT_BUNDLE_SILMA_TTS=1 npm run desktop -- --bundles appimage
```

Use a single bundle while validating the sidecar. A full `npm run desktop` build
packages the multi-GB PyInstaller runtime into every configured Linux artifact
and can spend many minutes compressing `.deb`, `.rpm`, and AppImage outputs.
For release verification, run the full bundle matrix after the sidecar size and
distribution strategy are settled.

Then launch the installed package or AppImage with only the runtime feature/model
env vars, not Python worker env vars:

```bash
PAPERCUT_ENABLE_SILMA_TTS=1 \
PAPERCUT_SILMA_MODEL_DIR="$PWD/.cache/silma-tts" \
./path/to/Papercut*.AppImage
```

The diagnostics probe should report `pythonCommand: "<bundled>"`.

## Optional Runtime Pack Plan

Do not ship the SILMA Python/PyTorch runtime inside ordinary Papercut
installers. The PyInstaller onedir output is multi-GB and made AppImage
packaging fail at `linuxdeploy` after the app itself built successfully.

Production direction:

- keep the base desktop app small;
- download a platform-specific SILMA runtime pack only when the user enables
  SILMA;
- download SILMA model files separately, pinned and verified;
- store both under app data;
- validate the runtime with the existing worker `--self-test`/probe before
  marking it usable.

Runtime pack location:

```text
<app-data>/runtimes/silma/<runtime-id>/current/
```

Current Linux x64 CPU runtime id:

```text
linux-x64-cpu
```

Current Linux x64 expected worker path inside the runtime pack:

```text
silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu
```

SILMA worker launch order is now:

1. `PAPERCUT_SILMA_WORKER_BIN`;
2. explicit `PAPERCUT_SILMA_WORKER` or `PAPERCUT_SILMA_PYTHON`;
3. app-data runtime pack;
4. bundled resource from the packaging spike;
5. editable repo worker script.

Model status reports SILMA runtime availability separately from SILMA model-file
availability. This lets the UI say whether the missing piece is the runtime
pack, the model files, or both.

Current runtime-pack install slice:

- the SILMA install button can promote a prepared PyInstaller onedir into the
  app-data runtime-pack slot;
- `npm run package:silma-runtime` can archive the prepared onedir and emit a
  JSON manifest with SHA-256 and byte size;
- source lookup checks `src-tauri/tts/silma-runtime-packs.json` for the current
  platform runtime id;
- source lookup falls back to the local
  `sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/` build output;
- install copies the onedir into a cache staging directory, runs the worker
  `--self-test`, and atomically promotes it to `current/`;
- archive install downloads to cache, verifies SHA-256, extracts to the same
  staging directory, runs the same `--self-test`, and promotes the same way;
- archive downloads resume from the cached partial `.tar.bz2` when the server
  supports HTTP range requests. Extraction staging is still cleared on each
  attempt, so interrupted installs cannot become active runtimes;
- this is not the final public manifest yet. It validates the on-disk layout,
  hash-gated download path, and app-data launch path the release downloader will
  use.

Local runtime-pack install prep:

```bash
npm run prepare:silma-sidecar -- --self-test
npm run package:silma-runtime
```

Then launch the dev app without `PAPERCUT_SILMA_WORKER_BIN`; the SILMA install
button can copy that prepared runtime into app data.

The package command writes:

```text
sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/papercut-silma-runtime-linux-x64-cpu.tar.bz2
sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/papercut-silma-runtime-linux-x64-cpu.manifest.json
```

Release runtime-pack metadata lives in:

```text
src-tauri/tts/silma-runtime-packs.json
```

Copy the release artifact URL, `sha256`, and `archiveBytes` into that manifest.
The current checked-in entry has an empty URL, so public runtime download stays
disabled until real release metadata exists.

After uploading the archive, update the checked app manifest from the generated
artifact metadata:

```bash
npm run package:silma-runtime -- \
  --url "https://example.com/papercut-silma-runtime-linux-x64-cpu.tar.bz2" \
  --update-app-manifest
```

This rewrites the matching `runtimeId` entry in
`src-tauri/tts/silma-runtime-packs.json`.

The archive must extract so `current/` contains the expected worker path:

```text
silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu
```

## Desktop Platform Notes

macOS:

- sign every Mach-O binary and dylib in the sidecar directory;
- include sidecar signing in the existing macOS release flow;
- notarize the final app;
- validate both Apple Silicon and Intel builds;
- expect PyTorch/Python signing wrinkles.

Windows:

- build a Windows sidecar on Windows;
- sign the sidecar executable;
- test installer upgrade behavior;
- expect occasional antivirus false positives with Python-packaged ML apps.

Linux:

- build on a conservative distro/container;
- validate AppImage library loading;
- check whether bundled PyTorch libraries conflict with system libraries;
- keep the worker isolated from sherpa shared libraries.

## Rust Integration Tasks

- [x] Add an explicit model backend/family field that can represent sherpa and
      SILMA without overloading `SherpaModelFamily`.
- [x] Split generic model family/backend identity from sherpa-only family
      selection.
- [x] Change `NativeTtsState` from `Option<SherpaTtsEngine>` to a backend-aware
      loaded engine slot.
- [x] Keep one loaded engine at a time unless measurements prove switching is too
      costly.
- [x] Route the save loop to sherpa or SILMA based on model definition.
- [x] Move shared WAV commit/validation/silent-placeholder code out of
      `synth.rs`.
- [x] Add a minimal `SilmaSidecar` JSONL process wrapper for local worker
      request/response calls.
- [x] Add `LoadedTtsEngine::Silma` and a dev `load_model` route.
- [x] Add SILMA chunk synthesis through the shared WAV sink.
- [ ] Add `SilmaSidecar` process supervision:
      - spawn;
      - health check;
      - load model;
      - synthesize;
      - timeout;
      - kill on app shutdown or backend switch;
      - recover after crash.
- [ ] Keep save cancellation cooperative between chunks at first.
- [ ] Add deterministic per-chunk seed support if SILMA output varies too much.
- [ ] Add diagnostics fields for sidecar startup, model load, worker memory if
      easy, synthesis time, and worker crashes.

Catalog scaffold status: `ModelDefinition` now has separate `backend`,
user-visible `family`, and `sherpa_family` fields. Existing sherpa models keep
their public IDs and installed paths, while future SILMA entries can use
`TtsModelBackend::SilmaSidecar`, `TtsModelFamily::SilmaF5`, and the
`models/silma-tts` storage prefix without pretending to be a sherpa family.

Runtime slot status: `NativeTtsState` now stores one `LoadedTtsEngine` enum.
`Sherpa` and `Silma` now both route through the native save loop. `Silma`
starts the worker, calls `load_model`, keeps the sidecar resident, and asks it
to synthesize missing chunk WAVs.

Hidden SILMA catalog status:

- Model id: `silma-ai/silma-tts`
- Backend: `TtsModelBackend::SilmaSidecar`
- Family: `TtsModelFamily::SilmaF5`
- Storage prefix: `models/silma-tts`
- Dev flag: `PAPERCUT_ENABLE_SILMA_TTS`

The entry is not advertised in normal capabilities. With the dev flag set on a
desktop build it can appear in the catalog. The install button installs the
missing SILMA piece in order: runtime pack first, then pinned model files.

Model status now detects a manually populated
`models/silma-tts/silma-tts/` directory, reports the expected local path when
files are missing, and offers the SILMA-specific installer instead of the
sherpa archive downloader.

Model status also detects whether a SILMA runtime is available from dev env
settings, the app-data runtime pack, the packaging-spike bundled resource, or
the editable repo worker. The UI surfaces a SILMA-only runtime-missing message
instead of claiming the voice model alone makes SILMA ready.

During development, `PAPERCUT_SILMA_MODEL_DIR` can point at the official SILMA
Hugging Face cache root, for example `./.cache/silma-tts`. The status/runtime
checks search that directory recursively for `model.pt` and `vocab.txt`, because
the official downloader stores files under `models--silma-ai--silma-tts/...`.
The app-owned installer stores those files directly under
`models/silma-tts/silma-tts/`.

## Python Worker Tasks

- [x] Create a minimal `silma_worker.py` that supports JSONL health/load/synth.
- [x] Load `SilmaTTS` once and keep it resident.
- [x] Accept a model/cache directory instead of relying on the default global
      Hugging Face cache.
- [x] Write output WAV to the exact path provided by the caller.
- [x] Return duration, sample rate, byte size, and synthesis timing.
- [x] Keep protocol on stdout and logs on stderr.
- [x] Add a local `--smoke` command that loads SILMA and synthesizes one WAV.
- [x] Run the local `--smoke` command with installed SILMA deps and capture
      timing/quality results.
- [x] Add a model-free probe WAV operation for sidecar/Tauri file-access testing.
- [x] Add a PyInstaller onefile prep script for the first spike.
- [x] Run and reject the PyInstaller onefile output on a desktop.
- [x] Package with PyInstaller onedir for production validation.
- [ ] Document exact Python version and locked dependencies.
- [ ] Decide whether `ffmpeg` is required at runtime; if yes, bundle it or avoid
      the code path that needs it.

Stage 0 worker location:

- `sidecars/silma/silma_worker.py`
- `sidecars/silma/README.md`
- `sidecars/silma/requirements.txt`

The worker imports `SilmaTTS` lazily so `health` and `--self-test` can run
without installing SILMA or downloading the model. `load_model` passes
`model_dir` as `hf_cache_dir` because the current official API resolves
`model.pt` and `vocab.txt` through `cached_path`.

If `--smoke` returns `No module named 'silma_tts'`, the selected Python
interpreter does not have the sidecar dependencies installed. Create/activate
the sidecar venv and run `pip install -r sidecars/silma/requirements.txt`, then
rerun the smoke command with that Python.

Local SILMA smoke command:

```bash
python sidecars/silma/silma_worker.py \
  --smoke \
  --model-dir ./.cache/silma-tts \
  --output-wav ./.cache/silma-tts-smoke.wav \
  --text "أنا نموذج سلمى لتحويل النص إلى كلام." \
  --seed 1234
```

This prints a JSON summary with model load timing, synthesis timing, output WAV
metadata, and real-time factor. It may download model files on first run.

Local smoke result, Python 3.12 venv on CPU:

- Model/cache dir: `./.cache/silma-tts`
- Downloaded SILMA snapshot: `d2515317033803648ecb8844765db9e583afecf9`
- Downloaded model weights: `model.pt` reconstructed to 2.60 GB
- Downloaded Vocos weights: `pytorch_model.bin` reconstructed to 54.4 MB
- Sample rate: 24,000 Hz
- First load: 208,599 ms
- Synthesis: 75,380 ms for 3.638 s of audio
- Real-time factor: 20.72 on CPU
- Output WAV: 174,676 bytes

The run warned that `ffmpeg`/`avconv` was not found, but this short reference
path still completed. Keep the packaging decision open until sidecar packaging
and longer reference-audio cases are tested.

Packaged `load_model` note: SILMA imports `transformers.pipeline` through
Transformers' lazy export path. The worker now imports
`transformers.pipelines.pipeline` directly and the packaging script marks
`transformers.pipelines` as a hidden import so PyInstaller includes that lazy
module path. The package script also excludes top-level `torchcodec`; SILMA does
not use that optional Transformers audio/video decoder path, and PyInstaller can
otherwise freeze enough of it for Transformers to detect it without freezing its
distribution metadata. If `load_model` still fails, check the sidecar stderr
traceback before adding more PyInstaller includes.

After changing worker/package imports, rebuild the packaged worker before
testing the app:

```bash
npm run prepare:silma-sidecar -- --clean --self-test
printf '%s\n' \
  '{"id":"load_model","op":"load_model","model_dir":"./.cache/silma-tts"}' \
  | sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu
```

To reuse this downloaded cache from Rust dev commands:

```bash
PAPERCUT_SILMA_MODEL_DIR=./.cache/silma-tts
PAPERCUT_SILMA_PYTHON=./.venv-silma/bin/python
```

Stage 1 probe command:

- Tauri command: `tts_probe_silma_sidecar`
- Rust module: `src-tauri/src/native_tts/engine/sidecar_probe.rs`
- Process wrapper: `src-tauri/src/native_tts/engine/silma_sidecar.rs`
- Worker op: `write_probe_wav`
- Frontend trigger: diagnostics-only `Probe Sidecar` button for the SILMA model
  in `AudioSetupPanel`
- Env overrides:
  - `PAPERCUT_SILMA_PYTHON`
  - `PAPERCUT_SILMA_WORKER`
  - `PAPERCUT_SILMA_WORKER_BIN`

The probe starts the Python worker from the repo, sends `health`, asks the
worker to write a tiny silent WAV into Tauri app data, validates that WAV with
Papercut's existing WAV parser, then sends `shutdown`. It is deliberately not a
SILMA model-load test. This probe uses Rust-owned `std::process::Command`
instead of Tauri's shell plugin. The process wrapper now keeps stdin/stdout open
for sequential JSONL request/response calls; production sidecar packaging,
timeouts, and crash recovery remain later stages.
`PAPERCUT_SILMA_WORKER_BIN` can point at a packaged PyInstaller worker
executable and skips Python entirely.

Packaged Linux onedir result:

- Command: `npm run prepare:silma-sidecar -- --clean --self-test`
- Platform: Linux x86_64, Python 3.12.3, PyInstaller 6.21.0
- Output: `sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/`
- Size: about 5.7 GB
- Packaged `--self-test`: passed

Desktop packaged-worker probe command for a dev run:

```bash
PAPERCUT_ENABLE_SILMA_TTS=1 \
PAPERCUT_SILMA_WORKER_BIN="$PWD/sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu" \
npm run tauri:dev
```

`npm run desktop` builds the installer. If the `.deb` is installed and launched
from the desktop environment, it will not inherit the shell env vars used during
the build. For an installed Linux package, launch the installed binary from a
terminal with the same env vars, or add them to a temporary wrapper script while
this remains a dev-gated feature.

In the app, enable TTS diagnostics and click
`Probe Sidecar`. A passing run logs `[tts-native] SILMA sidecar probe passed`
with the worker path, health version, probe WAV path, sample rate, duration, and
byte size.

Packaged desktop probe result:

- Worker: packaged PyInstaller onedir executable
- Health version: `0.1.0`
- Probe WAV path:
  `~/.local/share/io.papercut.desktop/silma-sidecar-probe/probe.wav`
- Sample rate: 24,000 Hz
- Duration: 0.25 s
- WAV bytes: 12,044
- Result: passed from the Tauri UI diagnostics button

Vite dev server note: the sidecar venv and packaged runtime are intentionally
ignored by `vite.config.ts` file watching. Without that, Linux can hit the
inotify watch limit while Vite scans `.venv-silma`.

The SILMA model itself is hidden unless `PAPERCUT_ENABLE_SILMA_TTS=1` is present
when the app process starts. Once visible, choose Arabic in the language control
and `SILMA Arabic TTS` in the model control.

Stage 2 load helper:

- Runtime slot variant: `LoadedTtsEngine::Silma`
- Loader: `ensure_silma_engine`
- Worker request: `load_model`
- Model dir override: `PAPERCUT_SILMA_MODEL_DIR`

This keeps the worker resident after `load_model` and lets the save loop call
the worker's `synthesize` op for missing chunks. The output still commits
through Rust's shared WAV sink, so manifest/cache/export/playback can keep using
the same saved-audiobook files as sherpa.

The JSONL worker redirects third-party stdout from SILMA/Nemo/Transformers to
stderr while handling requests. Rust reads stdout as strict one-line JSON, so
model logs such as `Preloading nemo normalizers ...` must never appear on the
protocol stream.

Rust passes a `.tmp` staging path to the sidecar because all audiobook backends
commit chunks through the shared atomic WAV sink. SILMA's Python writer infers
format from the filename extension, so the worker writes to an internal
`.tmp.wav` path and then renames it back to Rust's requested `.tmp` path before
returning.

Stage 2 SILMA synthesis status:

- Save loop selects sherpa or SILMA from `ModelDefinition.backend`.
- SILMA uses `TextPreprocessor` identity in Rust; the Python worker runs its own
  SILMA normalization/tashkeel path.
- SILMA validates the selected Papercut voice id, then ignores the numeric
  speaker id because SILMA uses the reference voice path instead.
- The existing thread selector is honored for SILMA by applying it to PyTorch
  CPU inference threads before model construction. Changing the thread count
  reloads the SILMA worker.
- The UI exposes a SILMA-only quality selector for F5 diffusion steps:
  `16` (default), `12`, `8`, and `4`. Lower values are for CPU benchmarking and
  may reduce quality.
- Native diagnostics include the SILMA backend label with device, PyTorch
  threads, inter-op threads, preprocessor, and NFE step.
- Dev-preview end-to-end saves now work with the source Python worker:

  ```bash
  PAPERCUT_ENABLE_SILMA_TTS=1 \
  PAPERCUT_SILMA_MODEL_DIR="$PWD/.cache/silma-tts" \
  PAPERCUT_SILMA_PYTHON="$PWD/.venv-silma/bin/python" \
  PAPERCUT_SILMA_WORKER="$PWD/sidecars/silma/silma_worker.py" \
  npm run tauri:dev
  ```

  This validates the app pipeline, not release packaging. Production still needs
  bundled sidecar discovery, in-app model install, dependency locking, and
  platform-specific package validation.

## Frontend Tasks

- [ ] Add SILMA model metadata to the TypeScript fallback catalog only after Rust
      capabilities can advertise it.
- [x] Keep SILMA hidden by default and expose it only behind a desktop/dev
      catalog flag.
- [ ] Show SILMA in ordinary UI only when desktop sidecar support is complete.
- [ ] Keep Android/iOS hidden or disabled with a clear native-unavailable reason.
- [ ] Represent SILMA voices as reference voice profiles.
- [ ] Ensure saved-audio filtering continues to include model id, voice id,
      speed, and text preprocessor.
- [ ] Add install/download copy that warns about model size before download.
- [ ] Add diagnostic labels that distinguish `sherpa-onnx-*` from
      `silma-sidecar`.
- [x] Surface SILMA runtime-pack status separately from model-file status.
- [x] Show SILMA-only CPU tuning controls when the SILMA model is selected:
      PyTorch threads through the existing thread selector and F5 NFE step
      through a compact quality selector.
- [x] Hide the install button for model entries that the current app installer
      cannot download.

## Build Script Tasks

- [x] Add `scripts/prepare-silma-sidecar.js` or platform-specific helpers.
- [x] Produce target-triple sidecar names or resource directories.
- [x] Add `onedir` packaging mode and make it the default after onefile failed
      extraction.
- [x] Add an optional packaged-worker `--self-test` to the prep script.
- [x] Force-include the Transformers pipeline module needed by packaged
      `load_model`.
- [x] Exclude optional `torchcodec` from the packaged worker after Transformers
      detected it without metadata.
- [x] Integrate sidecar prep into `scripts/build-desktop.js` behind a feature or
      build flag.
- [x] Add opt-in Linux desktop resource staging for the PyInstaller onedir
      sidecar with `PAPERCUT_BUNDLE_SILMA_TTS=1`.
- [x] Allow packaging spikes to request one Tauri bundle type with
      `npm run desktop -- --bundles appimage`.
- [x] Add app-data SILMA runtime-pack detection before implementing runtime
      downloads.
- [ ] Keep Android and iOS build scripts untouched except for explicit exclusion.
- [ ] Add CI smoke checks for sidecar presence in desktop bundles.
- [ ] Extend macOS verification to check sidecar signing.
- [ ] Extend release notes/build docs with SILMA desktop-only support.

## Security And Policy Tasks

- [ ] Pin and verify SILMA model sources.
- [ ] Store downloaded models under app data, not arbitrary user paths.
- [ ] Validate all paths sent to the sidecar are app-owned paths.
- [ ] Do not pass shell-interpreted arguments.
- [ ] Do not expose a generic "run command" permission to the frontend.
- [ ] Treat voice cloning as consent-sensitive.
- [ ] Ship only owned/licensed reference voice profiles in the first release.
- [ ] Add user-facing disclosure that SILMA audio is AI-generated if exported.
- [ ] Review SILMA code, model, CATT, NeMo, PyTorch, torchaudio, and ffmpeg
      licenses before distribution.

## Test Plan

Proof tests:

- [x] worker health returns version;
- [x] worker loads model from a local dir;
- [x] worker writes one valid WAV;
- [x] Rust validates and commits that WAV;
- [x] save loop can generate one short document with SILMA;
- [x] saved playback works from the existing chunk cache;
- [ ] export bundle and export WAV work without SILMA installed afterward.

Performance tests:

- [ ] cold worker startup time;
- [ ] model load time;
- [ ] warm chunk real-time factor;
- [ ] compare SILMA NFE `16`, `12`, `8`, and `4` on the same Arabic sample;
- [ ] compare SILMA PyTorch thread counts `1`, `2`, `4`, and detected max on
      the same Arabic sample;
- [ ] peak memory;
- [ ] package size;
- [ ] sidecar runtime size after excluding unused packaged dependencies;
- [ ] long Arabic chapter save;
- [ ] cancellation between chunks;
- [ ] worker crash recovery.

Platform tests:

- [ ] macOS Apple Silicon dev build;
- [ ] macOS Intel release build if still supported;
- [ ] Windows x64 release build;
- [ ] Linux AppImage/deb/rpm behavior;
- [ ] app upgrade with existing downloaded SILMA model;
- [ ] app upgrade with existing downloaded SILMA runtime pack;
- [ ] app uninstall leaves/removes user data according to existing policy.

Regression tests:

- [ ] Kokoro save/playback still works;
- [ ] Piper save/playback still works;
- [ ] Supertonic save/playback still works;
- [ ] Android native TTS build still ignores SILMA;
- [ ] iOS native TTS build still ignores SILMA.

## Stage Plan

### Stage 0: Feasibility Spike

- [x] Build a Python worker outside Tauri.
- [x] Synthesize one WAV with a local SILMA checkout/model.
- [x] Measure CPU RTF on at least one real desktop.
- [x] Decide whether quality and speed justify app integration.

Exit criteria:

- one valid 24 kHz-ish WAV;
- acceptable voice quality;
- known package size estimate;
- known warm CPU RTF range.

### Stage 1: Sidecar Prototype

- [x] Add PyInstaller onefile packaging helper.
- [x] Package worker with PyInstaller onefile and run the packaged worker.
- [x] Package worker with PyInstaller onedir and run the packaged worker.
- [x] Add temporary Rust command or dev-only path to spawn it.
- [x] Generate one probe WAV into app data.
- [x] Add diagnostics UI trigger for the packaged-worker probe.
- [x] Validate Tauri sidecar mechanics in a running desktop app.

Exit criteria:

- app can call sidecar from Rust;
- sidecar can write app-owned WAV;
- no frontend UI changes yet.

Current verification note: direct worker protocol checks pass, including
`write_probe_wav`. A full `cargo check --features native-tts-shared` on this
Linux machine is currently blocked before app code by missing
`javascriptcoregtk-4.1.pc`, a Tauri/WebKitGTK system dependency.

### Stage 2: Backend Integration

- [x] Add backend-aware loaded engine enum.
- [x] Add SILMA model catalog entry behind desktop feature gate.
- [x] Route save loop through shared synth-to-file boundary.
- [x] Reuse manifest/cache/export/playback.
- [x] Validate source-worker dev preview with completed SILMA save/playback jobs.

Exit criteria:

- saving a small document with SILMA creates a complete saved audiobook;
- Kokoro/Piper/Supertonic regressions pass.

### Stage 3: Model Install

- [x] Add SILMA model status/manual local-file detection.
- [x] Add SILMA runtime-pack status detection.
- [x] Add local SILMA runtime-pack install/promotion into app data.
- [x] Add manifest-backed SILMA runtime-pack archive download, SHA-256 verification,
      extraction, self-test, and promotion.
- [x] Add local SILMA runtime-pack archive packaging with a SHA-256 manifest.
- [x] Resume interrupted SILMA runtime-pack archive downloads when the server
      supports range requests.
- [x] Add SILMA runtime-pack download support behind checked release metadata.
- [x] Add release helper to update checked runtime metadata from the packaged
      artifact.
- [ ] Fill checked SILMA runtime-pack release metadata.
- [x] Add SILMA model-file in-app install support.
- [x] Pin model-file source revision and hashes.
- [x] Download model files to app data.
- [x] Validate required files.

Exit criteria:

- user can install SILMA from the app;
- interrupted/failed install does not leave an active partial model.

### Stage 4: Production Packaging

- [x] Switch from onefile to onedir after onefile extraction failed.
- [x] Add opt-in Linux resource staging and runtime discovery for the onedir
      sidecar.
- [x] Add AppImage-only packaging validation so the Linux spike does not rebuild
      every package format while iterating.
- [x] Decide to keep SILMA out of ordinary desktop installers and use an
      optional runtime pack instead.
- [ ] Bundle sidecar dependencies correctly on each desktop OS.
- [ ] Reduce sidecar size or move SILMA to an optional runtime download before
      enabling it in ordinary release installers.
- [ ] Sign/notarize macOS bundle.
- [ ] Sign Windows sidecar/app.
- [ ] Verify Linux bundles.

Exit criteria:

- clean install works on each supported desktop OS;
- release artifacts include sidecar checks.

### Stage 5: Product Polish

- [ ] Add final model/voice UI copy.
- [ ] Add download size warnings.
- [ ] Add diagnostics and troubleshooting docs.
- [ ] Add release notes.
- [ ] Add voice consent/export disclosure.

Exit criteria:

- feature is understandable without devtools;
- failures are recoverable by ordinary users.

## Open Questions

- What is acceptable warm CPU real-time factor for full audiobook generation?
- Which desktop platforms get SILMA in the first public build?
- Which reference voices are legally safe to ship?
- Does SILMA need ffmpeg for the exact inference path we will use?
- Can we avoid bundling the Gradio/web app dependencies entirely?
- Should SILMA downloads be individual upstream files or a Papercut-controlled
  archive of pinned upstream files?
- Does PyInstaller onedir fit cleanly into the Tauri bundle layout on all three
  desktop platforms?
- Do we need resumable downloads before first release because the model is much
  larger than current sherpa archives?

## Source References

- Tauri sidecars: https://v2.tauri.app/develop/sidecar/
- Tauri shell plugin: https://v2.tauri.app/plugin/shell/
- PyInstaller: https://pyinstaller.org/
- SILMA model card: https://huggingface.co/silma-ai/silma-tts
- SILMA repository: https://github.com/SILMA-AI/silma-tts
