#!/usr/bin/env bash
set -euo pipefail

APP_ID="${PAPERCUT_APP_ID:-io.papercut.desktop}"
TORCH_INDEX_URL="${PAPERCUT_SILMA_TORCH_INDEX_URL:-https://download.pytorch.org/whl/cu126}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
SILMA_ROOT="${PAPERCUT_SILMA_RUNTIME_ROOT:-$DATA_HOME/$APP_ID/runtimes/silma}"
RUNTIME_DIR="$SILMA_ROOT/linux-x64-cuda/current"
ENV_DIR="$RUNTIME_DIR/env"
MICROMAMBA_ROOT="$SILMA_ROOT/micromamba"
MICROMAMBA_BIN="${PAPERCUT_MICROMAMBA_BIN:-$SILMA_ROOT/tools/bin/micromamba}"
MANIFEST_PATH="$SILMA_ROOT/silma-runtime.local.json"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
WORKER_SOURCE="${PAPERCUT_SILMA_WORKER_SOURCE:-$REPO_ROOT/sidecars/silma/silma_worker.py}"

# Print a step marker that stays readable when pip starts being chatty.
log() {
  printf '[silma-cuda] %s\n' "$*"
}

# Fail with a short actionable message; this script is meant to be user-run.
die() {
  printf '[silma-cuda] error: %s\n' "$*" >&2
  exit 1
}

# Keep this NVIDIA-only for now; ROCm/XPU need their own tested install lanes.
require_linux_nvidia() {
  [[ "$(uname -s)" == "Linux" ]] || die 'SILMA CUDA setup is supported on Linux only.'
  [[ "$(uname -m)" == "x86_64" ]] || die 'SILMA CUDA setup is supported on Linux x64 only.'
  command -v nvidia-smi >/dev/null 2>&1 || die 'nvidia-smi was not found. Install the NVIDIA driver first.'
  local gpu_list
  gpu_list="$(nvidia-smi -L)" || die 'No NVIDIA GPU was reported by nvidia-smi.'
  [[ -n "$gpu_list" && "$gpu_list" != *"No devices were found"* ]] || die 'No NVIDIA GPU was reported by nvidia-smi.'
}

# Download the tiny conda-compatible package manager that will own Python and FFmpeg for this runtime.
install_micromamba() {
  if [[ -x "$MICROMAMBA_BIN" ]]; then
    return
  fi
  if command -v micromamba >/dev/null 2>&1; then
    MICROMAMBA_BIN="$(command -v micromamba)"
    return
  fi

  local tmp
  tmp="$(mktemp -d)"
  log "Downloading micromamba into $SILMA_ROOT/tools"
  if command -v curl >/dev/null 2>&1; then
    curl -Ls https://micro.mamba.pm/api/micromamba/linux-64/latest | tar -xj -C "$tmp" bin/micromamba
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- https://micro.mamba.pm/api/micromamba/linux-64/latest | tar -xj -C "$tmp" bin/micromamba
  else
    die 'curl or wget is required to download micromamba.'
  fi
  mkdir -p "$(dirname "$MICROMAMBA_BIN")"
  mv "$tmp/bin/micromamba" "$MICROMAMBA_BIN"
  rm -rf "$tmp"
  chmod 755 "$MICROMAMBA_BIN"
}

# Micromamba is a tiny conda-compatible package manager; use it to avoid system Python/FFmpeg drift.
create_python_env() {
  MAMBA_ROOT_PREFIX="$MICROMAMBA_ROOT" "$MICROMAMBA_BIN" create -y -p "$ENV_DIR" -c conda-forge \
    python=3.12 \
    ffmpeg \
    pip
}

# Run inside the managed env without shell activation, which keeps this script non-invasive.
env_python() {
  MAMBA_ROOT_PREFIX="$MICROMAMBA_ROOT" "$MICROMAMBA_BIN" run -p "$ENV_DIR" python "$@"
}

# The wrapper gives Papercut one stable executable while keeping Python source files available.
write_launcher() {
  local launcher="$RUNTIME_DIR/run-silma-worker"
  cat > "$launcher" <<'SH'
#!/usr/bin/env sh
set -eu
SELF="$0"
DIR=$(CDPATH= cd -- "$(dirname -- "$SELF")" && pwd)
PY="$DIR/env/bin/python"
LIB_PATH="$DIR/env/lib"
for EXTRA_LIB_DIR in \
  "$DIR"/env/lib/python*/site-packages/torch/lib \
  "$DIR"/env/lib/python*/site-packages/torchaudio/lib \
  "$DIR"/env/lib/python*/site-packages/nvidia/*/lib; do
  if [ -d "$EXTRA_LIB_DIR" ]; then
    LIB_PATH="$LIB_PATH:$EXTRA_LIB_DIR"
  fi
done
export PYTHONNOUSERSITE=1
# Keep micromamba's FFmpeg executable and native libs ahead of system packages.
export PATH="$DIR/env/bin${PATH:+:$PATH}"
export LD_LIBRARY_PATH="$LIB_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$PY" "$DIR/worker/silma_worker.py" "$@"
SH
  chmod 755 "$launcher"
}

# Verify that PyTorch sees a real CUDA device before Papercut is allowed to use this runtime.
write_manifest() {
  env_python - "$MANIFEST_PATH" <<'PY'
import json
import pathlib
import subprocess
import sys
import time

import torch

if not torch.cuda.is_available():
    raise SystemExit("torch.cuda.is_available() is false")

manifest_path = pathlib.Path(sys.argv[1])
driver = subprocess.run(
    ["nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader"],
    check=False,
    text=True,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
).stdout.strip().splitlines()

manifest_path.parent.mkdir(parents=True, exist_ok=True)
manifest_path.write_text(
    json.dumps(
        {
            "runtimeId": "linux-x64-cuda-local",
            "workerPath": "linux-x64-cuda/current/run-silma-worker",
            "device": "cuda",
            "torch": torch.__version__,
            "cuda": torch.version.cuda,
            "gpu": torch.cuda.get_device_name(0),
            "driver": driver[0] if driver else "",
            "environment": "micromamba",
            "installedAt": int(time.time()),
        },
        ensure_ascii=False,
        indent=2,
    )
    + "\n",
    encoding="utf-8",
)
PY
}

require_linux_nvidia
[[ -f "$WORKER_SOURCE" ]] || die "SILMA worker source was not found at $WORKER_SOURCE"

log "Installing runtime under: $RUNTIME_DIR"
log "Using micromamba root: $MICROMAMBA_ROOT"
log "Using PyTorch wheel index: $TORCH_INDEX_URL"

rm -rf "$RUNTIME_DIR"
mkdir -p "$RUNTIME_DIR/worker"
cp "$WORKER_SOURCE" "$RUNTIME_DIR/worker/silma_worker.py"

install_micromamba
create_python_env
env_python -m pip install -r "$REPO_ROOT/sidecars/silma/requirements.txt"
env_python -m pip install --upgrade --force-reinstall \
  torch torchvision torchaudio torchcodec \
  --index-url "$TORCH_INDEX_URL"
env_python -m pip uninstall -y onnxruntime-gpu
env_python -m pip install --upgrade --force-reinstall --no-deps onnxruntime

write_launcher
"$RUNTIME_DIR/run-silma-worker" --self-test
"$RUNTIME_DIR/run-silma-worker" --dependency-check
write_manifest

log "Installed SILMA CUDA runtime manifest: $MANIFEST_PATH"
log "Papercut will use this runtime before the downloaded CPU runtime pack."
