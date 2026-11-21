import {
  dirname,
  join,
} from "node:path"

import {
  SHERPA_REQUIRED_LIBS,
  SHERPA_VERSION,
} from "../constants.js"
import { fromRuntime } from "../paths.js"
import { nativeCacheDir } from "../native-paths.js"

export { SHERPA_REQUIRED_LIBS, SHERPA_VERSION }

export const SHERPA_LINUX_SHARED_SOURCE_DIR = process.env.SHERPA_ONNX_LIB_DIR || nativeCacheDir(
  "sherpa-onnx-prebuilt",
  "sherpa-onnx-v" + SHERPA_VERSION + "-linux-x64-shared-lib",
)
export const SHERPA_LINUX_CUDA_ARCHIVE_NAME =
  "sherpa-onnx-v" + SHERPA_VERSION + "-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-linux-x64-gpu.tar.bz2"
export const SHERPA_LINUX_CUDA_LIB_DIR = nativeCacheDir("sherpa-onnx-prebuilt", SHERPA_LINUX_CUDA_ARCHIVE_NAME.replace(".tar.bz2", ""))
export const SHERPA_LINUX_CUDA_ROOT = dirname(SHERPA_LINUX_CUDA_LIB_DIR)
export const SHERPA_LINUX_PREBUILT_ROOT = dirname(SHERPA_LINUX_CUDA_ROOT)
export const SHERPA_LINUX_CUDA_ARCHIVE = join(
  SHERPA_LINUX_PREBUILT_ROOT,
  SHERPA_LINUX_CUDA_ARCHIVE_NAME,
)
export const SHERPA_LINUX_CUDA_URL =
  "https://github.com/k2-fsa/sherpa-onnx/releases/download/v" +
  SHERPA_VERSION +
  "/" +
  SHERPA_LINUX_CUDA_ARCHIVE_NAME
export const SHERPA_LINUX_CUDA_SHA256 =
  "2aa541a65c1e7d921a053283419b70b30c1f6ae0a0f52f64b947ebe88c5f2a77"
export const SHERPA_LINUX_CUDA_PROVIDER_LIBS = [
  "libonnxruntime_providers_shared.so",
  "libonnxruntime_providers_cuda.so",
]
export const SHERPA_LINUX_SHARED_OUT_DIR = fromRuntime("linux-x64-shared-libs-" + SHERPA_VERSION)
export const SHERPA_LINUX_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.so"]
