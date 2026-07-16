#!/usr/bin/env bash
set -euo pipefail

APP_ID="${PAPERCUT_APP_ID:-io.papercut.desktop}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
SILMA_ROOT="${PAPERCUT_SILMA_RUNTIME_ROOT:-$DATA_HOME/$APP_ID/runtimes/silma}"
RUNTIME_INSTALLS_DIR="$SILMA_ROOT/linux-x64-cuda/installs"
INSTALL_ID="runtime-$(date -u +%Y%m%dT%H%M%SZ)-$$"
RUNTIME_DIR="$RUNTIME_INSTALLS_DIR/$INSTALL_ID"
ENV_DIR="$RUNTIME_DIR/env"
MICROMAMBA_ROOT="$SILMA_ROOT/micromamba"
MICROMAMBA_BIN="$SILMA_ROOT/tools/bin/micromamba"
MANIFEST_PATH="$SILMA_ROOT/silma-runtime.local.json"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
WORKER_SOURCE="${PAPERCUT_SILMA_WORKER_SOURCE:-$REPO_ROOT/sidecars/silma/silma_worker.py}"
MICROMAMBA_VERSION="2.8.1-0"
MICROMAMBA_URL="https://github.com/mamba-org/micromamba-releases/releases/download/$MICROMAMBA_VERSION/micromamba-linux-64"
MICROMAMBA_SHA256="9689782d863c05a1bf5d2d371ba527104e7a4eb4310c1637d8653b751aed9c82"
TORCH_INDEX_URL="https://download.pytorch.org/whl/cu126"
TORCH_VERSION="2.13.0+cu126"
TORCHVISION_VERSION="0.28.0+cu126"
TORCHAUDIO_VERSION="2.11.0+cu126"
TORCHCODEC_VERSION="0.15.0+cu126"
ONNXRUNTIME_VERSION="1.27.0"

# Print a step marker that stays readable when pip starts being chatty.
log() {
  printf '[silma-cuda] %s\n' "$*"
}

# Fail with a short actionable message; this script is meant to be user-run.
die() {
  printf '[silma-cuda] error: %s\n' "$*" >&2
  exit 1
}

# Remove only the inactive candidate when setup fails; the prior manifest/runtime stay usable.
cleanup_inactive_install() {
  rm -rf "$RUNTIME_DIR"
}

trap cleanup_inactive_install EXIT

# Keep this NVIDIA-only for now; ROCm/XPU need their own tested install lanes.
require_linux_nvidia() {
  [[ "$(uname -s)" == "Linux" ]] || die 'SILMA CUDA setup is supported on Linux only.'
  [[ "$(uname -m)" == "x86_64" ]] || die 'SILMA CUDA setup is supported on Linux x64 only.'
  command -v nvidia-smi >/dev/null 2>&1 || die 'nvidia-smi was not found. Install the NVIDIA driver first.'
  local gpu_list
  gpu_list="$(nvidia-smi -L)" || die 'No NVIDIA GPU was reported by nvidia-smi.'
  [[ -n "$gpu_list" && "$gpu_list" != *"No devices were found"* ]] || die 'No NVIDIA GPU was reported by nvidia-smi.'
}

# Download one verified app-owned micromamba so system package-manager drift cannot alter setup.
install_micromamba() {
  command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required to verify micromamba.'
  if [[ -x "$MICROMAMBA_BIN" ]] && printf '%s  %s\n' "$MICROMAMBA_SHA256" "$MICROMAMBA_BIN" | sha256sum --check --status; then
    return
  fi

  local tmp="$MICROMAMBA_BIN.download"
  mkdir -p "$(dirname "$MICROMAMBA_BIN")"
  rm -f "$tmp"
  log "Downloading micromamba $MICROMAMBA_VERSION into $SILMA_ROOT/tools"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 -o "$tmp" "$MICROMAMBA_URL"
  elif command -v wget >/dev/null 2>&1; then
    wget -O "$tmp" "$MICROMAMBA_URL"
  else
    die 'curl or wget is required to download micromamba.'
  fi
  printf '%s  %s\n' "$MICROMAMBA_SHA256" "$tmp" | sha256sum --check --status \
    || die 'Downloaded micromamba failed SHA-256 verification.'
  mv "$tmp" "$MICROMAMBA_BIN"
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
unset PYTHONHOME PYTHONPATH PYTHONUSERBASE PYTHONPLATLIBDIR PYTHONEXECUTABLE PYTHONSTARTUP
export PYTHONNOUSERSITE=1
# Keep micromamba's FFmpeg executable and native libs ahead of system packages.
export PATH="$DIR/env/bin${PATH:+:$PATH}"
export LD_LIBRARY_PATH="$LIB_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$PY" "$DIR/worker/silma_worker.py" "$@"
SH
  chmod 755 "$launcher"
}

# Verify CUDA, then atomically publish the only file Papercut uses for runtime discovery.
write_manifest() {
  local worker_path="linux-x64-cuda/installs/$INSTALL_ID/run-silma-worker"
  env_python - "$MANIFEST_PATH" "$worker_path" <<'PY'
import json
import pathlib
import subprocess
import sys
import time

import torch

if not torch.cuda.is_available():
    raise SystemExit("torch.cuda.is_available() is false")

manifest_path = pathlib.Path(sys.argv[1])
worker_path = sys.argv[2]
driver = subprocess.run(
    ["nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader"],
    check=False,
    text=True,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
).stdout.strip().splitlines()

manifest_path.parent.mkdir(parents=True, exist_ok=True)
pending_path = manifest_path.with_name(manifest_path.name + ".installing")
pending_path.write_text(
    json.dumps(
        {
            "runtimeId": "linux-x64-cuda-local",
            "workerPath": worker_path,
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
pending_path.replace(manifest_path)
PY
}

# Delete superseded environments only after the new manifest is active and validated.
remove_superseded_runtimes() {
  local candidate
  for candidate in "$RUNTIME_INSTALLS_DIR"/* "$SILMA_ROOT/linux-x64-cuda/current"; do
    [[ -e "$candidate" ]] || continue
    [[ "$candidate" == "$RUNTIME_DIR" ]] || rm -rf "$candidate"
  done
}

require_linux_nvidia
[[ -f "$WORKER_SOURCE" ]] || die "SILMA worker source was not found at $WORKER_SOURCE"

log "Installing runtime under: $RUNTIME_DIR"
log "Using micromamba root: $MICROMAMBA_ROOT"
log "Using pinned PyTorch CUDA 12.6 packages"

mkdir -p "$RUNTIME_DIR/worker"
cp "$WORKER_SOURCE" "$RUNTIME_DIR/worker/silma_worker.py"

install_micromamba
create_python_env
env_python -m pip install \
  "torch==$TORCH_VERSION" \
  "torchvision==$TORCHVISION_VERSION" \
  "torchaudio==$TORCHAUDIO_VERSION" \
  "torchcodec==$TORCHCODEC_VERSION" \
  --index-url "$TORCH_INDEX_URL"
env_python -m pip install -r "$REPO_ROOT/sidecars/silma/requirements.txt"
env_python -m pip uninstall -y onnxruntime-gpu
env_python -m pip install --force-reinstall --no-deps "onnxruntime==$ONNXRUNTIME_VERSION"

write_launcher
"$RUNTIME_DIR/run-silma-worker" --self-test
"$RUNTIME_DIR/run-silma-worker" --dependency-check
# From here on the candidate is valid; leave it inactive rather than deleting it if manifest publication fails.
trap - EXIT
write_manifest
remove_superseded_runtimes

log "Installed SILMA CUDA runtime manifest: $MANIFEST_PATH"
log "Papercut will use this runtime before the downloaded CPU runtime pack."
