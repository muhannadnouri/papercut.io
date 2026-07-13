# SILMA TTS Python Sidecar Development Guide

Last updated: 2026-07-12

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
- model install path assumes `models/sherpa-onnx`;
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
- consider resumable/range downloads after the first successful desktop spike;
- store under `models/silma-tts/<model-directory>/`, not under
  `models/sherpa-onnx`.

Required files are expected to include at least:

- `model.pt`
- `vocab.txt`
- `config.yaml`
- any SILMA/F5 inference config files required by the packaged runtime

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

Move silent-placeholder writing out of sherpa-specific code before SILMA uses the
save loop. A tiny PCM WAV writer in Rust is enough.

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

- PyInstaller `onefile` sidecar;
- easiest match for Tauri `externalBin`;
- slower startup and temp extraction;
- acceptable only for the first proof.

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

- [ ] Add an explicit model backend/family field that can represent sherpa and
      SILMA without overloading `SherpaModelFamily`.
- [ ] Split generic model metadata from sherpa-only load metadata.
- [ ] Change `NativeTtsState` from `Option<SherpaTtsEngine>` to a backend-aware
      loaded engine slot.
- [ ] Keep one loaded engine at a time unless measurements prove switching is too
      costly.
- [ ] Make `ensure_engine` route to sherpa or SILMA based on model definition.
- [ ] Move shared WAV commit/validation/silent-placeholder code out of
      `synth.rs`.
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

## Python Worker Tasks

- [x] Create a minimal `silma_worker.py` that supports JSONL health/load/synth.
- [x] Load `SilmaTTS` once and keep it resident.
- [x] Accept a model/cache directory instead of relying on the default global
      Hugging Face cache.
- [x] Write output WAV to the exact path provided by the caller.
- [x] Return duration, sample rate, byte size, and synthesis timing.
- [x] Keep protocol on stdout and logs on stderr.
- [ ] Add a tiny local smoke test that synthesizes one WAV from a known model dir.
- [ ] Package with PyInstaller onefile for the first spike.
- [ ] Package with PyInstaller onedir for production validation.
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

## Frontend Tasks

- [ ] Add SILMA model metadata to the TypeScript fallback catalog only after Rust
      capabilities can advertise it.
- [ ] Show SILMA only when desktop sidecar support is available.
- [ ] Keep Android/iOS hidden or disabled with a clear native-unavailable reason.
- [ ] Represent SILMA voices as reference voice profiles.
- [ ] Ensure saved-audio filtering continues to include model id, voice id,
      speed, and text preprocessor.
- [ ] Add install/download copy that warns about model size before download.
- [ ] Add diagnostic labels that distinguish `sherpa-onnx-*` from
      `silma-sidecar`.

## Build Script Tasks

- [ ] Add `scripts/prepare-silma-sidecar.js` or platform-specific helpers.
- [ ] Produce target-triple sidecar names or resource directories.
- [ ] Integrate sidecar prep into `scripts/build-desktop.js` behind a feature or
      build flag.
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

- [ ] worker health returns version;
- [ ] worker loads model from a local dir;
- [ ] worker writes one valid WAV;
- [ ] Rust validates and commits that WAV;
- [ ] save loop can generate one short document with SILMA;
- [ ] saved playback works from the existing chunk cache;
- [ ] export bundle and export WAV work without SILMA installed afterward.

Performance tests:

- [ ] cold worker startup time;
- [ ] model load time;
- [ ] warm chunk real-time factor;
- [ ] peak memory;
- [ ] package size;
- [ ] long Arabic chapter save;
- [ ] cancellation between chunks;
- [ ] worker crash recovery.

Platform tests:

- [ ] macOS Apple Silicon dev build;
- [ ] macOS Intel release build if still supported;
- [ ] Windows x64 release build;
- [ ] Linux AppImage/deb/rpm behavior;
- [ ] app upgrade with existing downloaded SILMA model;
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
- [ ] Synthesize one WAV with a local SILMA checkout/model.
- [ ] Measure CPU RTF and memory on at least one real desktop.
- [ ] Decide whether quality and speed justify app integration.

Exit criteria:

- one valid 24 kHz-ish WAV;
- acceptable voice quality;
- known package size estimate;
- known warm CPU RTF range.

### Stage 1: Sidecar Prototype

- [ ] Package worker with PyInstaller onefile.
- [ ] Add temporary Rust command or dev-only path to spawn it.
- [ ] Generate one WAV into app data.
- [ ] Validate shell/plugin/Tauri sidecar mechanics.

Exit criteria:

- app can call sidecar from Rust;
- sidecar can write app-owned WAV;
- no frontend UI changes yet.

### Stage 2: Backend Integration

- [ ] Add backend-aware loaded engine enum.
- [ ] Add SILMA model catalog entry behind desktop feature gate.
- [ ] Route save loop through shared synth-to-file boundary.
- [ ] Reuse manifest/cache/export/playback.

Exit criteria:

- saving a small document with SILMA creates a complete saved audiobook;
- Kokoro/Piper/Supertonic regressions pass.

### Stage 3: Model Install

- [ ] Add SILMA model status/install support.
- [ ] Pin source revision and hashes.
- [ ] Download to app data.
- [ ] Validate required files.

Exit criteria:

- user can install SILMA from the app;
- interrupted/failed install does not leave an active partial model.

### Stage 4: Production Packaging

- [ ] Switch from onefile to onedir if needed.
- [ ] Bundle sidecar dependencies correctly on each desktop OS.
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
