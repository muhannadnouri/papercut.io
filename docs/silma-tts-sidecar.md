# SILMA TTS Python Sidecar Development Guide

Last updated: 2026-08-01

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
- Ship SILMA only on Linux x64 until the upstream Python dependency stack
  installs cleanly on Windows and macOS.
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

Reference-voice models, such as SILMA/F5, use the reference WAV and its matching
transcript as a voice/style prompt at generation time. Built-in speaker models,
such as Kokoro, expose fixed voices from the model package; selecting a Kokoro
voice does not provide a new example recording to the model.

For the proof of concept, use SILMA's sample reference only if its license and
redistribution terms are acceptable for development use.

Current implementation exposes one voice profile because the packaged
`silma_tts` runtime only includes one reference sample:
`infer/ref_audio_samples/ar.ref.24k.wav`. Additional SILMA "voices" should be
modeled as more reference profiles, not speaker ids. Each profile needs a
reference WAV, matching reference text, stable id, display name, and explicit
license/consent metadata.

Reference quality notes:

- The reference transcript should match the audio exactly; changing text without
  changing the audio can make results worse.
- A better reference recording can improve voice/style consistency when it is
  clean, natural, not clipped, and close to the target language/accent/style.
- More sophisticated prose is not automatically better. Useful coverage is
  phonetic/prosodic, not literary.
- For the upstream `ar.ref.24k.wav` sample, keep the reference transcript
  identical to SILMA's official undiacritized example. SILMA estimates generated
  duration from reference-audio length and reference-text length, so adding
  diacritics to that transcript without changing the WAV can compress output and
  make words easier to skip.
- For custom Arabic references, an accurate matching transcript matters more
  than literary sophistication. Use diacritics only when the transcript still
  faithfully matches the reference audio and improves conditioning in listening
  tests.
- Keep first-release reference audio as WAV. SILMA's `pydub` import can warn
  when the `ffmpeg` executable is missing during editable-worker smoke tests,
  but release runtime packs still bundle FFmpeg shared libraries for TorchCodec.
  User-imported mp3/m4a/webm reference audio would be a separate feature that
  should explicitly transcode before reaching SILMA.

For production, use owned or explicitly licensed reference audio. Do not ship a
"clone any voice" UI in the first release. That is a product/legal feature, not
just an engineering switch. Treat SILMA's upstream bundled sample voice as
development-only until upstream confirms that the reference WAV can be
redistributed and exposed as a default Papercut voice profile.

Current export disclosure:

- All saved-audiobook export options remind users to label AI-generated audio if
  shared.
- Exporting any saved audiobook opens the standard confirmation dialog before
  the native save dialog.
- SILMA adds a reference-voice permission warning because it uses a reference
  recording as part of generation.
- Exported Papercut bundles include `silmaNfeStep` metadata so imported SILMA
  audiobooks restore the selected quality setting and cache identity.
- Reopening an imported SILMA bundle restores that NFE value from native
  metadata before checking saved-audio availability.
- Shared policy wording lives in [ai-audio-use-policy.md](ai-audio-use-policy.md).

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

Official Tauri guidance treats Python code as an executable sidecar after it is
bundled, commonly with PyInstaller. If the sidecar is shipped inside the app,
Tauri expects the configured `bundle.externalBin` path to have a matching
target-triple-suffixed executable next to it, such as
`my-sidecar-x86_64-unknown-linux-gnu`. Rust can then launch that bundled sidecar
with `app.shell().sidecar("my-sidecar")`; JavaScript launch paths require shell
plugin permissions.

For SILMA, the ordinary release path is not `externalBin`. The PyTorch/Python
runtime is too large for every base installer, so production uses an optional
runtime pack stored in app data and spawned by Rust from an app-owned path. The
Tauri bundled-sidecar path remains useful for validation spikes only.

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
- removed from the build helper.

Production path:

- PyInstaller `onedir`;
- faster startup;
- fewer temp extraction problems;
- easier to inspect and sign dependencies;
- packaged as an optional runtime pack, not as a base app resource.

Do not assume `externalBin` alone copies PyInstaller's whole `onedir`. The
resource-bundled spike proved too large for the ordinary desktop installer, so
the release path is an app-data runtime pack.

Rejected Linux bundled-resource spike:

- `PAPERCUT_BUNDLE_SILMA_TTS=1 npm run desktop` temporarily staged the
  PyInstaller onedir worker into Tauri resources.
- The runtime was multi-GB and made installer builds slow and fragile.
- That path has been removed. Do not use `PAPERCUT_BUNDLE_SILMA_TTS`.

Current Rust worker launch order:

- `PAPERCUT_SILMA_WORKER_BIN` for direct packaged-worker testing;
- explicit `PAPERCUT_SILMA_WORKER` or `PAPERCUT_SILMA_PYTHON` for source-worker
  development;
- user-local runtime manifest installed under app data;
- app-data runtime pack installed by the app;
- editable repo worker script for local dev fallback.

## Optional Runtime Pack Plan

Do not ship the SILMA Python/PyTorch runtime inside ordinary Papercut
installers. The runtime is multi-GB and made AppImage packaging fail at
`linuxdeploy` after the app itself built successfully.

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

Runtime id:

```text
linux-x64-cpu
```

The checked app manifest currently installs `linux-x64-cpu` by default. CUDA is
installed by the current user-local setup script instead of a published
multi-GB runtime pack. The app discovers that installation through:

```text
<app-data>/runtimes/silma/silma-runtime.local.json
```

The local manifest points at a wrapper executable with either `workerPath` or
`worker`, relative to the manifest directory or absolute:

```json
{"runtimeId":"linux-x64-cuda-local","workerPath":"linux-x64-cuda/installs/<install-id>/run-silma-worker"}
```

Rust prefers this user-local runtime before the downloaded CPU runtime pack.
The setup script verifies `torch.cuda.is_available()` before atomically writing
the manifest. A malformed local manifest is reported when no runtime is
available, but it does not block a valid downloaded CPU fallback. The native
model catalog hides SILMA on Windows, macOS, Android, and iOS until the upstream
Python dependency stack has a supported install path there.

Expected worker launcher path inside runtime packs:

```text
silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu
```

That path is a tiny shell launcher. The runtime directory also contains a copied
Python prefix under `python/` and the source worker under `worker/`. This keeps
SILMA, NeMo, Pynini, and TorchScript source/native-extension lookups in a normal
Python package layout instead of freezing them into a PyInstaller executable.

SILMA worker launch order is now:

1. `PAPERCUT_SILMA_WORKER_BIN`;
2. explicit `PAPERCUT_SILMA_WORKER` or `PAPERCUT_SILMA_PYTHON`;
3. user-local runtime manifest;
4. app-data CPU runtime pack;
5. editable repo worker script.

Model status reports SILMA runtime availability separately from SILMA model-file
availability. This lets the UI say whether the missing piece is the runtime
pack, the model files, or both.

Current runtime-pack install slice:

- the SILMA install button can promote a prepared source-preserving Python
  runtime into the app-data runtime-pack slot;
- `npm run prepare:silma-sidecar` copies a full Python prefix, the source worker,
  and a launcher into the runtime `onedir`;
- `npm run package:silma-runtime` can archive the prepared runtime and emit a
  JSON manifest with SHA-256 and byte size. Runtime archives must use the
  `.tar.bz2` suffix so the generated `.manifest.json` cannot collide with the
  archive path;
- source lookup checks `src-tauri/tts/silma-runtime-packs.json` for the current
  platform runtime id;
- source lookup falls back to the local
  `sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/` build output;
- install copies the runtime directory into a cache staging directory, runs the
  worker `--self-test`, and atomically promotes it to `current/`;
- archive install downloads to cache, verifies SHA-256, extracts to the same
  staging directory, runs the same `--self-test`, and promotes the same way;
- archives larger than GitHub Release's per-file limit are published as
  numbered `.partNNN` assets. The app downloads each part, reassembles the
  original `.tar.bz2`, then verifies the original archive SHA-256 before
  extraction;
- archive downloads resume from the cached partial `.tar.bz2` when the server
  supports HTTP range requests. Split archive downloads resume each cached part.
  Failed install attempts clear extraction staging but keep the partial archive
  cache for retry. Extraction staging is still cleared on each attempt, so
  interrupted installs cannot become active runtimes;
- the checked manifest is the public install source. Each runtime entry pins the
  release asset URL(s), original archive byte size, and original archive
  SHA-256.

Local CPU runtime-pack install prep:

```bash
sudo apt-get install -y --no-install-recommends ffmpeg
python -m pip install -r sidecars/silma/requirements-build.txt
python -m pip install --upgrade --force-reinstall torch torchvision torchaudio torchcodec \
  --index-url https://download.pytorch.org/whl/cpu
python -m pip uninstall -y onnxruntime-gpu
python -m pip install --upgrade --force-reinstall --no-deps onnxruntime
npm run prepare:silma-sidecar -- --clean --self-test --import-check --dependency-check
npm run package:silma-runtime
```

`prepare:silma-sidecar` must run against a full Python prefix, not a virtual
environment, because release runtime packs need to include the interpreter,
stdlib, source packages, and native extensions.

CUDA runtime direction:

Do not publish a CUDA runtime pack from CI yet. GitHub-hosted runners can build
files, but they cannot prove RTX-class runtime behavior without an NVIDIA
driver/GPU. CUDA support should be installed locally by an explicit Linux NVIDIA
setup script that creates a Papercut-owned micromamba environment, installs
Python 3.12, FFmpeg, the matching PyTorch CUDA wheels, validates
`torch.cuda.is_available()`, then writes `silma-runtime.local.json`.

Micromamba is a small conda-compatible package manager. Papercut uses it here so
the CUDA runtime can have its own Python and FFmpeg without touching the user's
system Python, distro packages, or shell startup files.

Current repo command:

```bash
npm run install:silma-cuda-runtime
```

The script creates a tiny executable wrapper and writes the local runtime
manifest only after the CUDA probe passes. The app can then use GPU
automatically without environment variables. If the local CUDA runtime is
absent, the downloaded CPU runtime remains the fallback.

Current CUDA setup script:

- lives at `scripts/install-silma-cuda-runtime.sh`;
- requires Linux x64, `nvidia-smi`, network access, `sha256sum`, and
  `curl` or `wget`;
- downloads the app-owned micromamba `2.8.1-0` binary from its fixed release URL
  and verifies SHA-256 before executing it;
- builds each candidate under
  `<app-data>/runtimes/silma/linux-x64-cuda/installs/<install-id>/` while the
  prior manifest and runtime remain active;
- copies the checked-in `sidecars/silma/silma_worker.py` source into the local
  runtime so TorchScript/source lookups still work;
- uses micromamba to install Python 3.12, FFmpeg, and pip inside
  that versioned candidate;
- installs `silma-tts==1.0.5` plus the tested CUDA 12.6 package set:
  `torch==2.13.0+cu126`, `torchvision==0.28.0+cu126`,
  `torchaudio==2.11.0+cu126`, and `torchcodec==0.15.0+cu126`;
- keeps ONNX Runtime on CPU because CATT/tashkeel uses ONNX Runtime while SILMA
  generation uses PyTorch, and pins it to `onnxruntime==1.27.0`;
- runs `--self-test`, `--dependency-check`, and a CUDA availability probe before
  atomically replacing `silma-runtime.local.json`. Failed candidates are
  removed without disturbing the previously active runtime; superseded
  runtimes are removed only after successful activation;
- accepts `PAPERCUT_SILMA_RUNTIME_ROOT` and
  `PAPERCUT_SILMA_WORKER_SOURCE` for test machines.
- SILMA worker launches strip inherited AppImage loader variables and all
  inherited `PYTHON*` variables before starting the worker. The worker launchers
  then set their own Python/FFmpeg paths, which avoids Arch-like `/bin/sh`
  symbol lookup failures and AppImage Python state leaking into micromamba
  runtimes.

The package command writes a manifest plus either one archive or numbered parts
when the archive is too large for a single GitHub Release asset:

```text
sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/papercut-silma-runtime-linux-x64-cpu.tar.bz2.part001
sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/papercut-silma-runtime-linux-x64-cpu.tar.bz2.part002
sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/papercut-silma-runtime-linux-x64-cpu.manifest.json
```

The CI runtime-pack job writes the same assets under
`sidecars/silma/runtime/x86_64-unknown-linux-gnu/archive/`.

Release runtime-pack metadata lives in:

```text
src-tauri/tts/silma-runtime-packs.json
```

Copy the release artifact URL(s), `sha256`, and `archiveBytes` into that
manifest. The v1.8.0 checked-in entry points at the split Linux x64 runtime
pack attached to the v1.8.0 GitHub Release and pins its original archive byte
size and SHA-256.

After generating release assets, update the checked app manifest from the
generated artifact metadata:

```bash
npm run package:silma-runtime -- \
  --url "https://example.com/papercut-silma-runtime-linux-x64-cpu.tar.bz2" \
  --update-app-manifest
```

This rewrites the matching `runtimeId` entry in
`src-tauri/tts/silma-runtime-packs.json`.

CI runtime-pack build:

- `.github/workflows/silma-runtime.yml` builds the Linux x64 CPU source runtime
  pack as an Actions artifact without running `npm run desktop`;
- the CPU job forces PyTorch's CPU wheel index. PyTorch's install selector
  documents Linux pip compute-platform installs using `--index-url` for
  CPU/CUDA wheels. `torchcodec` must be installed from the same index as
  `torch`; otherwise a CPU pack can accidentally contain CUDA-linked
  TorchCodec libraries and fail later with missing `libc10_cuda.so`;
- the runtime-pack job installs Ubuntu's FFmpeg package and copies the versioned
  FFmpeg shared libraries into the optional SILMA runtime. TorchCodec needs
  those `libav*`/`libsw*` libraries even when Papercut only passes WAV reference
  audio;
- this avoids the `linuxdeploy` failure path because the Python/PyTorch runtime
  is never placed inside the AppImage/deb/rpm bundle;
- PR validation: `.github/workflows/ci.yml` also builds and uploads the Linux
  CPU runtime pack when SILMA runtime files, packaging scripts, runtime
  metadata, or related workflows changed, so artifacts can be downloaded before
  the manual workflow lands on the default branch;
- Windows and macOS runtime-pack CI is intentionally disabled for now. The
  upstream SILMA dependency chain currently fails before packaging on those
  platforms. See `docs/silma-runtime-dependency-report.md`. Do not re-enable
  Windows/macOS runtime packs or advertise SILMA there until upstream provides a
  supported dependency path;
- PR CI cancels older in-progress runs for the same PR so the expensive desktop,
  mobile, and SILMA runtime jobs do not keep burning minutes after a newer push;
- pass `tag` to make the generated manifest use the predictable GitHub Release
  URL;
- set `upload_to_release` only after the release exists and you want the
  workflow to attach the `.tar.bz2.partNNN` files and generated manifest to
  that release.
  Published runtime-pack assets are immutable; if an upload target already
  exists, create a new release/tag or delete the bad asset deliberately rather
  than overwriting it from CI;
- use the generated `.manifest.json` from CI to fill
  `src-tauri/tts/silma-runtime-packs.json` in a normal PR before enabling public
  runtime install for that release.

CUDA runtime policy:

- ship CPU first because it is universal on supported Linux x64 desktops;
- keep CUDA as the only GPU runtime lane for now because NVIDIA CUDA is the
  most common acceleration path for this model class;
- use a user-local CUDA setup script before attempting hosted CUDA runtime packs;
- keep ONNX Runtime on CPU in both runtime packs; SILMA's heavy generation path is
  PyTorch, while CATT/tashkeel uses ONNX Runtime during preprocessing;
- reinstall CPU ONNX Runtime with `--no-deps` so pip does not upgrade NumPy
  beyond the versions accepted by SILMA's librosa/numba stack;
- the generated launcher adds bundled `torch/lib`, `torchaudio/lib`, and
  `site-packages/nvidia/*/lib` directories to `LD_LIBRARY_PATH` so relocated
  PyTorch/torchaudio can find pip-installed native libraries;
- do not add ROCm, Intel XPU, Windows, or macOS runtime packs until there is a
  tester and an upstream-supported dependency path;
- do not re-enable hosted CUDA runtime-pack CI until there is a real NVIDIA
  validation machine. A non-GPU CI runner cannot prove `libcudart.so.12`,
  torchaudio, TorchCodec, and driver compatibility for end users.

Practical CI model:

- always run cheap PR checks for lint, types, frontend build, and basic native
  worker protocol coverage;
- gate expensive runtime-pack and platform packaging jobs from actual changed
  files, not branch names;
- run desktop/mobile packaging only after cheap checks pass and only when native
  runtime, platform, packaging, workflow, or dependency surfaces changed;
- keep full desktop/mobile/runtime artifact matrices for manual validation,
  nightly checks, or release workflows;
- use the manual `CI` workflow dispatch when a PR needs release-level confidence
  even though path detection would skip some expensive jobs;
- use PR concurrency cancellation so superseded pushes stop spending minutes.

Manual CI validation scopes:

- `cheap`: lint, frontend build, and worker self-tests only;
- `desktop`: cheap checks plus Linux, Windows, and macOS desktop packaging;
- `mobile`: cheap checks plus Android and iOS packaging checks;
- `silma-runtime`: cheap checks plus the Linux x64 SILMA runtime-pack artifact;
- `full`: all of the above.

Each archive must extract so `current/` contains the expected worker path for
that platform:

```text
silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu
```

## Diagnostics and Troubleshooting

When SILMA fails, turn on **TTS Diagnostics** in Audio Setup and copy the JSON.
Useful entries:

- `[tts-native] capabilities`: confirms the desktop backend is available and
  whether `silma-ai/silma-tts` is advertised.
- `[tts-native] model install started`: records the selected model, install
  support, runtime/model directories, and whether the click is starting with a
  SILMA runtime install.
- `[tts-native] model install completed`: records the final model/runtime status
  after the install action. A warning level means the action completed but the
  model is still not ready.
- `[tts-native] model install failed`: records the install error plus the last
  known runtime/model status.
- `[tts-native] SILMA sidecar probe passed`: confirms Rust can spawn the worker,
  exchange JSONL, and write a probe WAV.
- `[tts-save] native chunk start`: includes the backend label, model dir,
  detected Torch device, PyTorch thread count, text preprocessor, and NFE step.
- `[tts-save] failed`: usually carries the worker error response or Rust-side
  file validation error.

Known recovery paths:

- SILMA does not appear in the model list: confirm the app is a Linux x64 build
  from a commit that includes the public runtime metadata. Windows, macOS, and
  mobile builds currently hide SILMA.
- `SILMA runtime pack is not installed`: install the optional runtime pack first.
  If public download is disabled, fill `src-tauri/tts/silma-runtime-packs.json`
  from a release artifact or use the local packaged runtime flow.
- Model files are missing: click the SILMA model install button after the runtime
  is installed. The app downloads pinned `model.pt` and `vocab.txt` and verifies
  them before promotion.
- Probe passes but save fails: copy diagnostics and check the worker stderr. The
  most useful backend label fields are `device=...`, `torch_threads=...`,
  `torch_interop=...`, `preprocessor=...`, and `nfe_step=...`.
- `Failed to parse SILMA worker response`: third-party Python logs reached
  stdout. The worker must keep stdout as protocol-only JSONL and send logs to
  stderr.
- `No format specified ... .tmp`: the worker must write to an internal
  `.tmp.wav` path and rename it back to Rust's requested temp path.
- `ffmpeg`/`avconv` warning in editable-worker dev: expected on systems without
  the ffmpeg executable. Packaged runtime installs should include the FFmpeg
  shared libraries copied during `prepare:silma-sidecar`; missing `libav*`
  diagnostics mean the runtime pack is invalid.
- `device=cpu`: expected on machines without CUDA/MPS/XPU. NFE `32` is the
  app default because it matches the normal F5-TTS quality setting; reduce NFE
  for CPU speed tests or try `64` for slower quality checks. GPU acceleration
  depends on the packaged Torch build and available hardware.
- `device=unreported`: the worker did not return device metadata. Rebuild or
  reinstall the SILMA runtime pack from a current worker.
- Vite/Tauri `ENOSPC` watcher error: keep `.venv-silma`, `.cache`, packaged
  SILMA runtimes, and large test documents outside the watched tree when
  possible. It is safe to delete generated SILMA build output with
  `rm -rf .cache/silma-pyinstaller sidecars/silma/runtime dist`. Do not delete
  `src-tauri/tts/runtime` as a generic cleanup step; Tauri validates the sherpa
  Linux shared-lib resource path before bundling. If it was deleted, restore it
  with `node scripts/copy-sherpa-linux-libs.js` after Cargo has downloaded the
  sherpa shared libs.
- `linuxdeploy` failure while bundling: do not put the multi-GB SILMA runtime in
  ordinary app bundles. Use the optional runtime-pack artifact path.

## Reviewer Test Recipe

Minimal local review before release artifacts exist:

1. Build or reuse a local runtime pack:

   ```bash
   . .venv-silma/bin/activate
   python -m pip install -r sidecars/silma/requirements-build.txt
   npm run prepare:silma-sidecar -- --clean --self-test --import-check --dependency-check
   ```

2. Remove only SILMA app-data runtime/model folders, not saved audiobooks:

   ```bash
   rm -rf ~/.local/share/io.papercut.desktop/runtimes/silma
   rm -rf ~/.local/share/io.papercut.desktop/models/silma-tts
   ```

3. Launch the dev app:

   ```bash
   npm run tauri:dev
   ```

4. In Audio Setup, select `SILMA Arabic TTS`, enable diagnostics, click
   `Install SILMA`, then confirm diagnostics show model install start/completion.

5. Click `Probe Sidecar`, then save a short Arabic audiobook. Confirm the save
   diagnostics include `device`, `torch_threads`, `torch_interop`,
   `preprocessor`, and `nfe_step` in the backend label.

6. Quit and relaunch with `npm run tauri:dev`. SILMA should stay installed from
   app data without `PAPERCUT_SILMA_PYTHON`, `PAPERCUT_SILMA_WORKER`, or
   `PAPERCUT_SILMA_WORKER_BIN`.

## Desktop Platform Notes

Linux:

- build on a conservative distro/container;
- validate AppImage library loading;
- check whether bundled PyTorch libraries conflict with system libraries;
- keep the worker isolated from sherpa shared libraries.

Windows and macOS:

- SILMA is hidden on these platforms for now;
- dependency installation fails before packaging starts;
- use `docs/silma-runtime-dependency-report.md` when reporting upstream;
- revisit only after upstream documents a supported dependency path.

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
- [x] Add SILMA-specific save chunk sizing to avoid a second awkward split in
      the Python F5 runtime.
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

SILMA catalog status:

- Model id: `silma-ai/silma-tts`
- Backend: `TtsModelBackend::SilmaSidecar`
- Family: `TtsModelFamily::SilmaF5`
- Storage prefix: `models/silma-tts`
Linux x64 desktop builds advertise SILMA by default. Windows, macOS, and mobile
builds currently do not advertise it.
One install click installs the missing SILMA pieces in order: runtime pack
first, then pinned model files.

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
checks require the same Hugging Face cache layout that SILMA's own `cached_path`
loader uses:

```text
models/silma-tts/silma-tts/
  models--silma-ai--silma-tts/
    refs/main
    blobs/<cached blob ids>
    snapshots/d2515317033803648ecb8844765db9e583afecf9/
      model.pt
      vocab.txt
```

The app-owned installer writes that layout directly so the first real synthesis
run does not download the 2.5 GB SILMA checkpoint a second time. If an older
Papercut build already downloaded flat `model.pt` and `vocab.txt` files, the
installer can reuse those files while promoting the directory to Hugging Face
cache layout. Installed-state validation also requires `refs/main` to contain
the pinned revision shown above; snapshot files alone are not enough because
SILMA's `cached_path` request resolves the default Hugging Face branch.

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
- [x] Add and then remove a PyInstaller onefile prep path after the first spike.
- [x] Run and reject the PyInstaller onefile output on a desktop.
- [x] Package with PyInstaller onedir for production validation.
- [x] Document exact Python version and pinned runtime/build dependency inputs.
- [x] Decide whether `ffmpeg` is required at runtime; if yes, bundle it or avoid
      the code path that needs it.
- [x] Bundle direct FFmpeg shared libraries in the optional Linux SILMA runtime
      pack after TorchCodec proved it needs `libav*` at import time.

Stage 0 worker location:

- `sidecars/silma/silma_worker.py`
- `sidecars/silma/README.md`
- `sidecars/silma/requirements.txt`
- `sidecars/silma/requirements-build.txt`

Dependency inputs:

- CI runtime-pack builds use Python 3.12.
- `requirements.txt` pins the direct SILMA runtime dependency used by smoke
  tests and editable-worker dev runs.
- `requirements-build.txt` includes the runtime requirements and stays as the
  CI cache key/build input for source-preserving runtime-pack builds.
- Do not hand-maintain a transitive lock from a local machine. If the public
  runtime pack needs a fully hashed lock later, generate it from the release CI
  image that actually builds the pack.

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

The editable-worker run warned that `ffmpeg`/`avconv` was not found, but this
bundled-WAV reference path completed. Packaged runtimes are stricter: the
runtime pack bundles FFmpeg shared libraries because TorchCodec can fail before
synthesis when `libavutil`/related FFmpeg libraries are unavailable.

No-ffmpeg editable-worker validation command:

```bash
PATH="$PWD/.venv-silma/bin" \
.venv-silma/bin/python sidecars/silma/silma_worker.py \
  --smoke \
  --model-dir ./.cache/silma-tts \
  --output-wav ./.cache/silma-tts-no-ffmpeg.wav \
  --text "أنا نموذج سلمى لتحويل النص إلى كلام." \
  --seed 1234
```

Packaged-worker validation command:

```bash
SILMA_WORKER_DIR="$PWD/sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/silma-worker-x86_64-unknown-linux-gnu"
SILMA_WORKER="$SILMA_WORKER_DIR/silma-worker-x86_64-unknown-linux-gnu"
"$SILMA_WORKER" \
  --smoke \
  --model-dir ./.cache/silma-tts \
  --output-wav ./.cache/silma-tts-packaged.wav \
  --text "أنا نموذج سلمى لتحويل النص إلى كلام." \
  --seed 1234
```

Packaged import note: SILMA imports NeMo/Pynini native extensions and compiles
x-transformers helpers through TorchScript. The runtime pack now preserves the
normal Python source/package layout instead of freezing those modules with
PyInstaller. If `--import-check` or `--dependency-check` fails, treat the runtime
pack as invalid and fix the Python environment before publishing artifacts.
`--import-check` imports SILMA's public API. `--dependency-check` also imports
the packaged Torch, torchaudio, TorchCodec, and Transformers audio utility path
that caught the v1.7.3 CPU runtime's CUDA-linked TorchCodec mismatch.

After changing worker/package imports, rebuild the packaged worker before
testing the app:

```bash
npm run prepare:silma-sidecar -- --clean --self-test --import-check --dependency-check
```

`--self-test` proves the JSONL protocol and file-writing path. `--import-check`
imports SILMA's real API path inside the packaged worker without downloading
model weights. `--dependency-check` imports the native audio stack without
downloading model weights. These are the CI gates that should catch packaged
`x_transformers` TorchScript source lookup failures and CPU/CUDA wheel
mismatches before runtime artifacts are published.

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
`PAPERCUT_SILMA_WORKER_BIN` can point at the packaged runtime launcher and
skips repo Python discovery.

Packaged Linux runtime result:

- Command: `npm run prepare:silma-sidecar -- --clean --self-test --import-check --dependency-check`
- Platform: Linux x86_64, Python 3.12 full prefix
- Output: `sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/`
- Contents: launcher, copied Python prefix, and source `silma_worker.py`
- Packaged `--self-test`: required to pass
- Packaged `--import-check`: required to pass before publishing artifacts
- Packaged `--dependency-check`: required to pass before publishing artifacts

Desktop packaged-worker probe command for a dev run:

```bash
PAPERCUT_SILMA_WORKER_BIN="$PWD/sidecars/silma/runtime/x86_64-unknown-linux-gnu/onedir/silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu" \
npm run tauri:dev
```

`npm run desktop` builds the installer. If the `.deb` is installed and launched
from the desktop environment, it will not inherit the shell env vars used during
the build. Public SILMA installs do not require an env var; the packaged app
uses the checked runtime metadata.

In the app, enable TTS diagnostics and click
`Probe Sidecar`. A passing run logs `[tts-native] SILMA sidecar probe passed`
with the worker path, health version, probe WAV path, sample rate, duration, and
byte size.

Packaged desktop probe result:

- Worker: packaged source-preserving runtime launcher
- Health version: `0.1.0`
- Probe WAV path:
  `~/.local/share/io.papercut.desktop/silma-sidecar-probe/probe.wav`
- Sample rate: 24,000 Hz
- Duration: 0.25 s
- WAV bytes: 12,044
- Result: passed from the Tauri UI diagnostics button

Vite dev server note: the sidecar venv, packaged runtime, generated caches, root
HTML/EPUB test documents, and large public document tree are intentionally
ignored by `vite.config.ts` file watching. Without that, Linux can hit the
inotify watch limit while Vite scans generated SILMA files or local book
fixtures. Tauri has its own watcher too; if it reports
`OS file watch limit reached` for a normal source file such as
`src-tauri/Cargo.toml`, clean generated SILMA output first, then raise the Linux
inotify limit if the workspace is still too large. VSCodium/VS Code workspace
settings also exclude `src-tauri/target`, `.venv-silma`, `.cache`, and local book
fixtures from editor file watching; reload the editor window after changing
those excludes so old watchers are released.

In debug desktop builds, choose Arabic in the language control and
`SILMA Arabic TTS` in the model control. Release Linux x64 builds show the same
option.

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
- Frontend save chunking uses a smaller SILMA-only profile, currently 80
  characters max, because SILMA/F5 internally re-chunks around 120 characters
  after Arabic normalization and tashkeel. Keeping Papercut chunks shorter
  reduces skipped short words around punctuation and avoids odd pauses in short
  final clauses.
- SILMA validates the selected Papercut voice id, then ignores the numeric
  speaker id because SILMA uses the reference voice path instead.
- The existing thread selector is honored for SILMA by applying it to PyTorch
  CPU inference threads before model construction. Changing the thread count
  reloads the SILMA worker.
- The UI exposes a SILMA-only quality selector for F5 diffusion steps:
  `32` (default/recommended), `64`, `16`, `12`, `8`, and `4`. SILMA is built on
  F5-TTS, whose official demo/default material uses NFE `32`; lower values are
  for speed tests and may sound more synthetic.
- Native diagnostics include the SILMA backend label with the detected Torch
  device (`cpu`, `cuda`, `mps`, or `xpu`), PyTorch threads, inter-op threads,
  preprocessor, and NFE step. Older sidecars that do not report a device are
  shown as `unreported` rather than a guessed value.
- Dev-preview end-to-end saves now work with the source Python worker:

  ```bash
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
- [x] Show SILMA by default in debug desktop builds; keep release/package
      validation behind a flag until public runtime metadata is filled.
- [ ] Show SILMA in ordinary UI only when desktop sidecar support is complete.
- [ ] Keep Android/iOS hidden or disabled with a clear native-unavailable reason.
- [ ] Represent SILMA voices as reference voice profiles.
- [ ] Ensure saved-audio filtering continues to include model id, voice id,
      speed, and text preprocessor.
- [x] Add install/download copy that warns about model size before download.
- [x] Add diagnostic labels that distinguish `sherpa-onnx-*` from
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
- [x] Make the SILMA packaging helper build a source-preserving Python runtime
      after PyInstaller failed on native/source-inspection dependencies.
- [x] Add optional packaged-worker `--self-test` and `--import-check` gates to
      the prep script.
- [x] Add packaged-worker `--dependency-check` so CI imports TorchCodec and
      catches mismatched CPU/CUDA native wheels before publishing runtime packs.
- [x] Prototype sidecar prep in `scripts/build-desktop.js` behind a build flag.
- [x] Remove the opt-in Linux desktop resource staging spike after choosing
      optional runtime packs.
- [x] Allow packaging spikes to request one Tauri bundle type with
      `npm run desktop -- --bundles appimage`.
- [x] Add app-data SILMA runtime-pack detection before implementing runtime
      downloads.
- [x] Add user-local SILMA runtime manifest detection for future CUDA setup.
- [x] Add repo-run local SILMA CUDA setup script that writes that manifest after
      CUDA validation.
- [x] Move CUDA setup script Python and FFmpeg handling into a Papercut-local
      micromamba environment.
- [ ] Keep Android and iOS build scripts untouched except for explicit exclusion.
- [ ] Add CI smoke checks for Linux SILMA runtime-pack archives.
- [ ] Extend release notes/build docs with SILMA Linux-only support.

## Security And Policy Tasks

- [x] Pin and verify SILMA model sources.
- [x] Pin the CUDA compatibility stack and verify the downloaded micromamba
      bootstrap binary.
- [x] Store downloaded models under app data, not arbitrary user paths.
- [ ] Validate all paths sent to the sidecar are app-owned paths.
- [ ] Do not pass shell-interpreted arguments.
- [ ] Do not expose a generic "run command" permission to the frontend.
- [ ] Treat voice cloning as consent-sensitive.
- [ ] Ship only owned/licensed reference voice profiles in the first release.
- [ ] Confirm upstream SILMA sample-reference redistribution rights before
      shipping it as a default voice.
- [x] Add user-facing disclosure that exported audiobooks are AI-generated.
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
- [ ] compare SILMA NFE `64`, `32`, `16`, `12`, `8`, and `4` on the same Arabic sample;
- [ ] compare SILMA PyTorch thread counts `1`, `2`, `4`, and detected max on
      the same Arabic sample;
- [ ] peak memory;
- [ ] package size;
- [ ] sidecar runtime size after excluding unused packaged dependencies;
- [ ] long Arabic chapter save;
- [ ] cancellation between chunks;
- [ ] worker crash recovery.

Platform tests:

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

- [x] Add, run, reject, and remove PyInstaller onefile packaging.
- [x] Package worker with PyInstaller onedir, run it, and reject that format.
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
- [x] Keep partial runtime-pack archives after recoverable install failures so
      retry can resume instead of starting over.
- [x] Install missing SILMA runtime and model files from one user action.
- [x] Store SILMA model files in Hugging Face cache layout so SILMA's Python
      loader reuses the app download instead of fetching the checkpoint again.
- [x] Add SILMA runtime-pack download support behind checked release metadata.
- [x] Add release helper to update checked runtime metadata from the packaged
      artifact.
- [x] Add CI workflow to build the Linux SILMA runtime pack as a separate
      artifact.
- [x] Add a packaged-worker SILMA import check so CI catches TorchScript source
      lookup failures before publishing runtime artifacts.
- [x] Add a packaged-worker dependency check so CI catches TorchCodec/FFmpeg and
      CPU/CUDA wheel mismatches before publishing runtime artifacts.
- [x] Fill checked SILMA runtime-pack release metadata.
- [x] Split the Linux runtime pack into GitHub Release-safe asset parts and
      reassemble/verify it during install.
- [x] Add SILMA model-file in-app install support.
- [x] Pin model-file source revision and hashes.
- [x] Download model files to app data.
- [x] Validate required files and the pinned Hugging Face `refs/main` value.
- [x] Build local CUDA candidates off to the side and atomically activate their
      manifest only after runtime and CUDA checks pass.
- [x] Fall back to the downloaded CPU runtime when an optional local CUDA
      manifest is malformed.

Exit criteria:

- user can install SILMA from the app;
- interrupted/failed install does not leave an active partial model.

### Stage 4: Production Packaging

- [x] Switch from onefile to onedir after onefile extraction failed.
- [x] Remove opt-in Linux resource staging after it proved too large for the
      base installer.
- [x] Add AppImage-only packaging validation so the Linux spike does not rebuild
      every package format while iterating.
- [x] Decide to keep SILMA out of ordinary desktop installers and use an
      optional runtime pack instead.
- [x] Add separate CI packaging for the Linux SILMA runtime pack.
- [x] Cancel stale PR CI runs when a newer push arrives for the same PR.
- [x] Run the temporary PR SILMA runtime-pack build only when SILMA runtime
      files, packaging scripts, runtime metadata, or related workflows changed.
- [x] Split ordinary PR CI into cheap required checks and gated expensive
      packaging jobs.
- [x] Add manual PR validation scopes for cheap, desktop, mobile, SILMA runtime,
      or full CI.
- [x] Move SILMA to an optional runtime download instead of enabling it in
      ordinary release installers.
- [x] Bundle sidecar dependencies correctly in the Linux runtime pack.
- [x] Bundle FFmpeg shared libraries in the optional Linux SILMA runtime pack.
- [x] Rebuild the Linux CPU runtime pack with CPU `torchcodec` pinned from the
      PyTorch CPU wheel index and verify a clean install.
- [x] Add local-runtime discovery before building the Linux CUDA setup script.
- [ ] Verify Linux bundles.
- [ ] Revisit Windows/macOS runtime packs after upstream dependency fixes.

Exit criteria:

- clean install works on Linux x64;
- Linux release artifacts include sidecar checks.

### Stage 5: Product Polish

- [x] Add initial model/voice UI copy.
- [x] Add download size warnings for the SILMA runtime/model install path.
- [x] Add diagnostics and troubleshooting docs.
- [x] Add release notes.
- [x] Add voice consent/export disclosure.

Exit criteria:

- feature is understandable without devtools;
- failures are recoverable by ordinary users.

## Open Questions

- What is acceptable warm CPU real-time factor for full audiobook generation?
- Which desktop platforms get SILMA in the first public build?
- Which reference voices are legally safe to ship?
- Can we avoid bundling the Gradio/web app dependencies entirely?
- Should SILMA downloads be individual upstream files or a Papercut-controlled
  archive of pinned upstream files?
- Do we need resumable downloads before first release because the model is much
  larger than current sherpa archives?

## Source References

- Tauri sidecars: https://v2.tauri.app/develop/sidecar/
- Tauri shell plugin: https://v2.tauri.app/plugin/shell/
- PyInstaller: https://pyinstaller.org/
- SILMA model card: https://huggingface.co/silma-ai/silma-tts
- SILMA repository: https://github.com/SILMA-AI/silma-tts
