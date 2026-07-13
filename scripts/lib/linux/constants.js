import { join } from "node:path"
import { SHERPA_REQUIRED_LIBS, SHERPA_VERSION } from "../constants.js"
import { fromRuntime, SRC_TAURI_DIR } from "../paths.js"

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
