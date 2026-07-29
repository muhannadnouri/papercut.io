# SILMA Runtime Packaging Dependency Report

Date: 2026-07-13

## Summary

Papercut can package and run the SILMA Python sidecar runtime on Linux x64, but
Windows and macOS runtime packaging currently fail during Python dependency
installation before PyInstaller or Papercut code runs.

Papercut is therefore shipping SILMA only for Linux x64 desktop users for now.
Windows and macOS users will not see SILMA in the native TTS model catalog until
the upstream dependency stack has a supported install path on those platforms.

## How This Was Encountered

Papercut added CI jobs to build optional downloadable SILMA runtime packs. The
runtime pack intentionally stays separate from the main app installer so the
ordinary desktop app remains small and users download the large Python/PyTorch
runtime only if they enable SILMA.

The Linux x64 runtime-pack job succeeded. macOS and Windows failed while running:

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

The `silma-tts==1.0.5` wheel declares these relevant dependencies:

```text
nemo_text_processing==1.1.0
catt_tashkeel==1.0.2
torch>=2.0.0
torchaudio>=2.0.0
torchvision>=0.21
```

`nemo_text_processing==1.1.0` requires:

```text
pynini==2.1.6.post1
```

`catt_tashkeel==1.0.2` requires:

```text
onnxruntime-gpu>=1.22.0
```

## macOS Failure

On macOS runners, dependency installation fails because
`onnxruntime-gpu>=1.22.0` has no macOS wheel.

Minimal reproduction:

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

Observed result:

```text
ERROR: Could not find a version that satisfies the requirement onnxruntime-gpu==1.22.0
ERROR: No matching distribution found for onnxruntime-gpu==1.22.0
```

macOS x64 also hit wheel availability issues while resolving
`torchvision>=0.21`.

## Windows Failure

On Windows x64 GitHub Actions runners, dependency installation reaches
`pynini==2.1.6.post1` through `nemo_text_processing==1.1.0`. Pip then tries to
build Pynini from source and fails in MSVC:

```text
Failed to build installable wheels for some pyproject.toml based projects
pynini
cl : Command line error D8021 : invalid numeric argument '/Wno-register'
```

NVIDIA's NeMo text-processing documentation says pip installation is expected to
work on Linux x86_64, while Windows and macOS should use conda-forge for Pynini.
Pynini's own install instructions also point Windows and macOS users at
conda-forge.

Papercut is not pursuing a conda/micromamba runtime-pack path right now. The
product scope is Linux x64 only until upstream clarifies or changes the Windows
and macOS dependency story.

## Why This Blocks Papercut

Papercut's SILMA integration depends on producing a platform-specific runtime
archive that can be downloaded, hash-verified, extracted, and launched without
requiring users to install Python themselves.

PyInstaller is not a cross-compiler: a Windows runtime must be built on Windows,
a Linux runtime on Linux, and a macOS runtime on macOS. Docker is therefore not a
general fix for producing Windows/macOS PyInstaller artifacts from Linux.

Because Windows and macOS fail before packaging starts, Papercut cannot produce
trustworthy runtime packs for those platforms today.

## Suggested Upstream Fixes

These are upstream dependency packaging issues, not Papercut application-code
issues.

Useful upstream clarifications or fixes:

- document the supported Windows/macOS install path for `silma-tts`;
- make `catt_tashkeel` avoid unconditional `onnxruntime-gpu` on platforms where
  GPU ONNX Runtime is unavailable;
- make NeMo/CATT text processing optional if SILMA inference can run without it;
- publish or document a pip-compatible Windows/macOS dependency set, if pip is
  intended to be supported there.

For `catt_tashkeel`, the dependency likely needs platform markers similar to:

```text
onnxruntime-gpu>=1.22.0; platform_system != "Darwin"
onnxruntime>=1.22.0; platform_system == "Darwin"
```

The exact split should be decided upstream. Linux CPU packaging may also prefer
`onnxruntime` over `onnxruntime-gpu`.

## Current Papercut Behavior

Until upstream provides a clean path for Windows and macOS:

- SILMA is available only on Linux x64 desktop builds;
- SILMA is hidden from Windows, macOS, Android, and iOS native TTS catalogs;
- SILMA runtime-pack CI builds Linux x64 only;
- the checked runtime-pack manifest contains only the Linux x64 runtime entry.

