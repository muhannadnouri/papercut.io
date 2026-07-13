# SILMA macOS Runtime Packaging Dependency Report

Date: 2026-07-13

## Summary

Papercut can package the SILMA Python sidecar runtime on Linux x64 and Windows
x64, but macOS runtime packaging currently fails during dependency installation
before PyInstaller or Papercut code runs.

The likely root cause is platform-incompatible dependency metadata in the SILMA
runtime dependency chain. In particular, `catt_tashkeel` requires
`onnxruntime-gpu>=1.22.0`, but `onnxruntime-gpu` does not provide macOS wheels.

Papercut is therefore disabling the SILMA app option on macOS until the upstream
dependency stack installs cleanly on macOS runners.

## How This Was Encountered

Papercut added CI jobs to build optional downloadable SILMA runtime packs for:

- Linux x64 CPU
- Windows x64 CPU
- macOS x64 CPU
- macOS Apple Silicon CPU

The Linux runtime pack succeeded. The macOS runtime-pack jobs failed while
running:

```bash
python -m pip install --upgrade pip
python -m pip install -r sidecars/silma/requirements-build.txt
```

`sidecars/silma/requirements-build.txt` includes:

```text
-r requirements.txt
pyinstaller==6.21.0
```

`sidecars/silma/requirements.txt` includes:

```text
silma-tts==1.0.5
```

## Reproduction

On a macOS Python 3.12 environment, install SILMA's runtime dependency:

```bash
python -m venv .venv-silma
. .venv-silma/bin/activate
python -m pip install --upgrade pip
python -m pip install silma-tts==1.0.5 pyinstaller==6.21.0
```

Expected:

The dependency graph resolves using CPU-compatible macOS wheels.

Actual:

Dependency installation fails before packaging can start.

Observed failure examples from GitHub Actions macOS runners:

```text
ERROR: Could not find a version that satisfies the requirement onnxruntime-gpu>=1.22.0
ERROR: No matching distribution found for onnxruntime-gpu>=1.22.0
```

macOS x64 also hit wheel availability issues while resolving
`torchvision>=0.21`.

The `onnxruntime-gpu` macOS issue can be reproduced directly:

```bash
python -m pip download \
  --only-binary=:all: \
  --platform macosx_14_0_arm64 \
  --python-version 312 \
  --implementation cp \
  --abi cp312 \
  --no-deps \
  onnxruntime-gpu==1.22.0
```

Result:

```text
ERROR: Could not find a version that satisfies the requirement onnxruntime-gpu==1.22.0
ERROR: No matching distribution found for onnxruntime-gpu==1.22.0
```

## Why This Blocks Papercut

Papercut's SILMA integration uses an optional desktop runtime pack:

- the main app stays small;
- the Python/PyTorch runtime is downloaded only when users enable SILMA;
- the runtime archive is built in CI for each supported platform;
- the app verifies the runtime archive hash before installation.

That approach depends on producing a working, platform-specific PyInstaller
runtime pack. macOS packaging cannot proceed while the dependency graph requires
a package that has no macOS distribution.

## Suggested Upstream Fix

Make the SILMA dependency chain platform-aware so macOS CPU installs do not
require GPU-only ONNX Runtime packages.

For example, the dependency that currently requires `onnxruntime-gpu` likely
needs platform markers similar to:

```text
onnxruntime-gpu>=1.22.0; platform_system != "Darwin"
onnxruntime>=1.22.0; platform_system == "Darwin"
```

The exact marker split should be decided upstream, because Linux CPU packaging
may also prefer `onnxruntime` over `onnxruntime-gpu`.

## Current Papercut Behavior

Until the upstream macOS dependency installation succeeds:

- SILMA remains available on supported Linux x64 and Windows x64 desktop builds;
- SILMA is hidden from the macOS native TTS model catalog;
- macOS runtime-pack CI jobs are disabled;
- macOS runtime IDs remain documented as future placeholders.

