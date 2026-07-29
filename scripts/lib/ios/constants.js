import { join } from "node:path"
import { SHERPA_VERSION } from "../constants.js"
import { fromRuntime } from "../paths.js"

export { SHERPA_VERSION }

export const SHERPA_IOS_RUNTIME_ROOT = fromRuntime("sherpa-onnx-ios")
export const SHERPA_IOS_ARCHIVE_NAME = "sherpa-onnx-v" + SHERPA_VERSION + "-ios.tar.bz2"
export const SHERPA_IOS_ARCHIVE = join(SHERPA_IOS_RUNTIME_ROOT, SHERPA_IOS_ARCHIVE_NAME)
export const SHERPA_IOS_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v" +
  SHERPA_VERSION +
  "/" +
  SHERPA_IOS_ARCHIVE_NAME
export const SHERPA_IOS_SHA256 = "596f33bff80046a52144745745fe54d55e8b23659d92209f5ab7d94c1259fe6d"

export const SHERPA_IOS_DEVICE_SLICE = "ios-arm64"
export const SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE = "ios-arm64_x86_64-simulator"
export const SHERPA_IOS_SIMULATOR_ARM64_SLICE = "ios-arm64-simulator"
export const SHERPA_IOS_DEFAULT_SLICE = SHERPA_IOS_DEVICE_SLICE

export const SHERPA_IOS_BUILD_ROOT = join(SHERPA_IOS_RUNTIME_ROOT, "build-ios")
export const SHERPA_IOS_SHERPA_XCFRAMEWORK = join(SHERPA_IOS_BUILD_ROOT, "sherpa-onnx.xcframework")
// Upstream keeps this symlink pointed at the bundled ONNX Runtime version, so
// sherpa bumps do not also require a hardcoded ORT directory update here.
export const SHERPA_IOS_ONNXRUNTIME_XCFRAMEWORK = join(
  SHERPA_IOS_BUILD_ROOT,
  "ios-onnxruntime",
  "onnxruntime.xcframework",
)

export const SHERPA_IOS_RUST_LIBS = ["libsherpa-onnx.a", "libonnxruntime.a"]
