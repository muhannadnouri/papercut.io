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
export const SHERPA_LINUX_SHARED_OUT_DIR = fromRuntime("linux-x64-shared-libs-" + SHERPA_VERSION)
export const SHERPA_LINUX_OPTIONAL_LIBS = ["libsherpa-onnx-cxx-api.so"]
