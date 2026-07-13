import { join } from "node:path"
import { SHERPA_REQUIRED_LIBS, SHERPA_VERSION } from "../constants.js"
import { fromRuntime, ROOT, SRC_TAURI_DIR } from "../paths.js"

export { SHERPA_REQUIRED_LIBS, SHERPA_VERSION }

export const SHERPA_LINUX_SHARED_SOURCE_DIR = join(
  SRC_TAURI_DIR,
  "target",
  "sherpa-onnx-prebuilt",
  "sherpa-onnx-v" + SHERPA_VERSION + "-linux-x64-shared-lib",
  "lib",
)
export const SHERPA_LINUX_SHARED_OUT_DIR = fromRuntime("linux-x64-shared-libs")
export const SHERPA_LINUX_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.so"]

export const SILMA_LINUX_TARGET = "x86_64-unknown-linux-gnu"
export const SILMA_LINUX_EXE_BASE = "silma-worker-" + SILMA_LINUX_TARGET
export const SILMA_LINUX_SIDECAR_SOURCE_DIR = join(
  ROOT,
  "sidecars",
  "silma",
  "runtime",
  SILMA_LINUX_TARGET,
  "onedir",
  SILMA_LINUX_EXE_BASE,
)
export const SILMA_LINUX_SIDECAR_OUT_DIR = fromRuntime("silma-sidecar-linux-x64")
