import { join } from "node:path"
import { SHERPA_REQUIRED_LIBS, SHERPA_VERSION } from "../constants.js"
import { fromRuntime, SRC_TAURI_DIR } from "../paths.js"

export { SHERPA_REQUIRED_LIBS, SHERPA_VERSION }

export const SHERPA_LINUX_PREBUILT_ROOT = join(
  SRC_TAURI_DIR,
  "target",
  "sherpa-onnx-prebuilt",
)
export const SHERPA_LINUX_SHARED_SOURCE_DIR = join(
  SHERPA_LINUX_PREBUILT_ROOT,
  "sherpa-onnx-v" + SHERPA_VERSION + "-linux-x64-shared-lib",
  "lib",
)
export const SHERPA_LINUX_CUDA_ARCHIVE_NAME =
  "sherpa-onnx-v" + SHERPA_VERSION + "-cuda-12.x-cudnn-9.x-linux-x64-gpu.tar.bz2"
export const SHERPA_LINUX_CUDA_ARCHIVE = join(
  SHERPA_LINUX_PREBUILT_ROOT,
  SHERPA_LINUX_CUDA_ARCHIVE_NAME,
)
export const SHERPA_LINUX_CUDA_ROOT = join(
  SHERPA_LINUX_PREBUILT_ROOT,
  SHERPA_LINUX_CUDA_ARCHIVE_NAME.replace(".tar.bz2", ""),
)
export const SHERPA_LINUX_CUDA_LIB_DIR = join(SHERPA_LINUX_CUDA_ROOT, "lib")
export const SHERPA_LINUX_CUDA_URL =
  "https://github.com/k2-fsa/sherpa-onnx/releases/download/v" +
  SHERPA_VERSION +
  "/" +
  SHERPA_LINUX_CUDA_ARCHIVE_NAME
export const SHERPA_LINUX_CUDA_SHA256 =
  "2ba80dd4df761b8de58d578190846f6f2349523685e33bcb24f65ba586c43563"
export const SHERPA_LINUX_CUDA_PROVIDER_LIBS = [
  "libonnxruntime_providers_shared.so",
  "libonnxruntime_providers_cuda.so",
]
export const SHERPA_LINUX_SHARED_OUT_DIR = fromRuntime("linux-x64-shared-libs")
export const SHERPA_LINUX_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.so"]
