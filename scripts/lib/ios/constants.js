import { join } from "node:path"
import { SHERPA_VERSION } from "../constants.js"
import { fromRuntime } from "../paths.js"

export { SHERPA_VERSION }

export const SHERPA_IOS_RUNTIME_ROOT = fromRuntime("sherpa-onnx-ios-" + SHERPA_VERSION)
export const SHERPA_IOS_ARCHIVE_NAME = "sherpa-onnx-v" + SHERPA_VERSION + "-ios-static.xcframework.zip"
export const SHERPA_IOS_ARCHIVE = join(SHERPA_IOS_RUNTIME_ROOT, SHERPA_IOS_ARCHIVE_NAME)
export const SHERPA_IOS_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/" +
  "xcframework" +
  "/" +
  SHERPA_IOS_ARCHIVE_NAME
export const SHERPA_IOS_SHA256 = "88aa569f71b7903e0bbdfbe6e2e114b3b95862b1ef0759eea3024f82926d7374"

export const SHERPA_IOS_DEVICE_SLICE = "ios-arm64"
export const SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE = "ios-arm64_x86_64-simulator"
export const SHERPA_IOS_SIMULATOR_ARM64_SLICE = "ios-arm64-simulator"
export const SHERPA_IOS_DEFAULT_SLICE = SHERPA_IOS_DEVICE_SLICE

export const SHERPA_IOS_BUILD_ROOT = SHERPA_IOS_RUNTIME_ROOT
export const SHERPA_IOS_SHERPA_XCFRAMEWORK = join(SHERPA_IOS_BUILD_ROOT, "sherpa-onnx.xcframework")
export const SHERPA_IOS_ONNXRUNTIME_XCFRAMEWORK = join(SHERPA_IOS_BUILD_ROOT, "onnxruntime.xcframework")
export const ORT_IOS_ARCHIVE = join(SHERPA_IOS_RUNTIME_ROOT, "onnxruntime-1.28.2.zip")
export const ORT_IOS_URL = "https://github.com/csukuangfj/onnxruntime-libs/releases/download/v1.28.2/onnxruntime-ios-static-xcframework-1.28.2.xcframework.zip"
export const ORT_IOS_SHA256 = "2c2299acbb461d26d4bac4bc85985d40e7c7177ed6072703ae0846d88b0b4599"

export const SHERPA_IOS_RUST_LIBS = ["libsherpa-onnx.a", "libonnxruntime.a", "libpiper_phonemize.a"]
