# SILMA Worker

Desktop-only JSONL worker for the SILMA TTS sidecar.

Tauri can start this worker for diagnostics, runtime probes, model loading, and
audiobook chunk synthesis. It can also run standalone for local smoke tests and
packaged-runtime validation.

## Setup

Use Python 3.10+.

```bash
python -m venv .venv-silma
. .venv-silma/bin/activate
pip install -r sidecars/silma/requirements.txt
```

The current Papercut path uses SILMA's bundled WAV reference and writes WAV
output. Still run packaged `--dependency-check` before publishing runtime packs:
TorchCodec can require compatible FFmpeg/PyTorch native libraries even before a
full synthesis smoke test.

## Self-Test

This does not import or download SILMA.

```bash
python sidecars/silma/silma_worker.py --self-test
```

## Packaged Dependency Check

This imports SILMA plus the native audio stack that commonly fails only after
runtime packaging:

```bash
python sidecars/silma/silma_worker.py --dependency-check
```

## Manual Smoke

This imports SILMA, loads the model through the official `SilmaTTS` API, and
writes one WAV. The first run may download model files into the provided cache
directory.

If this fails with `No module named 'silma_tts'`, the current Python did not
install `sidecars/silma/requirements.txt`; activate `.venv-silma` or point the
app at that interpreter with `PAPERCUT_SILMA_PYTHON`.

Shortest form:

```bash
python sidecars/silma/silma_worker.py --smoke
```

Useful explicit form:

```bash
python sidecars/silma/silma_worker.py \
  --smoke \
  --model-dir ./.cache/silma-tts \
  --output-wav ./.cache/silma-tts-smoke.wav \
  --text "أنا نموذج سلمى لتحويل النص إلى كلام." \
  --seed 1234
```

No-ffmpeg validation for the editable worker:

```bash
PATH="$PWD/.venv-silma/bin" \
.venv-silma/bin/python sidecars/silma/silma_worker.py \
  --smoke \
  --model-dir ./.cache/silma-tts \
  --output-wav ./.cache/silma-tts-no-ffmpeg.wav \
  --text "أنا نموذج سلمى لتحويل النص إلى كلام." \
  --seed 1234
```

Packaged-worker validation:

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

JSONL form, matching the protocol Rust uses:

```bash
python sidecars/silma/silma_worker.py <<'JSONL'
{"id":"1","op":"health"}
{"id":"2","op":"load_model","model_dir":"./.cache/silma-tts"}
{"id":"3","op":"synthesize","text":"أنا نموذج سلمى لتحويل النص إلى كلام.","output_wav":"./.cache/silma-tts-smoke.wav","speed":1.0,"seed":1234}
{"id":"4","op":"shutdown"}
JSONL
```

The worker writes protocol responses to stdout and runtime logs to stderr.

## Tauri Probe

The Rust command `tts_probe_silma_sidecar` starts this worker from the repo,
performs a `health` request, asks it to write a tiny silent probe WAV into app
data, validates that WAV with the native audiobook WAV parser, and shuts the
worker down.

Optional environment overrides:

```bash
PAPERCUT_SILMA_PYTHON=/path/to/venv/bin/python
PAPERCUT_SILMA_WORKER=/path/to/silma_worker.py
PAPERCUT_SILMA_WORKER_BIN=/path/to/packaged/silma-worker
```

## Packaging Spike

Install the build requirements into a full Python prefix, then build a
source-preserving target-suffixed runtime:

```bash
sudo apt-get install -y --no-install-recommends ffmpeg
python -m pip install -r sidecars/silma/requirements-build.txt
npm run prepare:silma-sidecar -- --clean --self-test --import-check --dependency-check
```

The output is written under `sidecars/silma/runtime/<target>/onedir/`, which is
ignored by git. Point Rust at the executable with `PAPERCUT_SILMA_WORKER_BIN`.

The first onefile Linux spike produced a 3.18 GB executable, but packaged
`--self-test` failed while extracting `torch/lib/libtorch_cpu.so`, even with
`TMPDIR` pointed outside `/tmp`. The build helper now only supports onedir.

Release builds publish this as an optional runtime pack, not inside the base app
installer.

## Local CUDA Runtime

Linux NVIDIA users can create a user-local CUDA runtime that Papercut discovers
before the downloaded CPU runtime pack:

```bash
npm run install:silma-cuda-runtime
```

Prerequisites:

- Linux x64;
- working NVIDIA driver with `nvidia-smi`;
- network access;
- `sha256sum`;
- `curl` or `wget`.

Micromamba is a tiny conda-compatible environment manager. Here it gives
Papercut its own Python 3.12 and FFmpeg install under app data, without touching
your system Python or distro packages.

The script verifies a pinned app-owned micromamba binary, then creates an
isolated environment under
`~/.local/share/io.papercut.desktop/runtimes/silma/linux-x64-cuda/installs/`.
It installs Python 3.12, FFmpeg, SILMA, and a pinned CUDA 12.6 PyTorch stack,
verifies the worker and `torch.cuda.is_available()`, then atomically writes:

```text
~/.local/share/io.papercut.desktop/runtimes/silma/silma-runtime.local.json
```

The previous runtime remains active if installation fails. After activation,
older local CUDA environments are removed. Test machines can redirect the app
data root or worker source:

```bash
PAPERCUT_SILMA_RUNTIME_ROOT=/custom/runtimes/silma
PAPERCUT_SILMA_WORKER_SOURCE=/custom/silma_worker.py
```
