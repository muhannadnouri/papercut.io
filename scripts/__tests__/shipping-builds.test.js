import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"
import { ROOT } from "../lib/paths.js"
import {
  PAGEFIND_SOURCE_REVISION,
  PAGEFIND_WASM_PATH_PREFIX,
} from "../lib/android/constants.js"
import { createHash } from "node:crypto"
import { join } from "node:path"

const pagefindPath = join(ROOT, ".cache/android-production/source-pagefind/pagefind")
const pagefindReceiptPath = join(ROOT, ".cache/android-production/source-pagefind/source-receipt.json")
const pagefindBytes = Buffer.from("source-built Pagefind")
const calls = vi.hoisted(() => ({
  runSync: vi.fn(() => ({ status: 0 })),
  exitFromResult: vi.fn(),
  android: vi.fn(),
  androidLibs: vi.fn(),
  clearAndroidLibs: vi.fn(),
  iosLibs: vi.fn(),
  prepare: vi.fn(),
  resources: vi.fn(),
  pagefindReceipt: vi.fn(),
  pagefindExists: vi.fn(),
}))
vi.mock("../lib/build-revision.js", () => ({ productionBuildRevision: () => "cd".repeat(20) }))
vi.mock("../lib/process.js", () => ({
  runSync: calls.runSync,
  exitFromResult: calls.exitFromResult,
}))
vi.mock("../lib/tauri.js", () => ({
  tauriCommand: args => ({
    command: "tauri",
    args,
  }),
}))
vi.mock("../lib/desktop/platform.js", () => ({
  currentDesktopPlatform: () => ({
    linux: "linux",
    win32: "windows",
    darwin: "macos",
  })[process.platform],
  prepareDesktopBuild: calls.prepare,
  desktopBuildEnv: (_platform, env) => env,
  prepareDesktopBundleResources: calls.resources,
}))
vi.mock("../lib/android/build-env.js", () => ({
  runTauriAndroidBuild: calls.android,
  androidProductionEnv: () => ({
    RUSTUP_TOOLCHAIN: "1.98.1",
    PAPERCUT_BUILD_REVISION: "cd".repeat(20),
    CARGO_ENCODED_RUSTFLAGS: "production remaps",
  }),
}))
vi.mock("../lib/android/sherpa.js", () => ({
  androidSherpaLibDir: () => "/prepared/android",
  clearAndroidSherpaLibs: calls.clearAndroidLibs,
  ensureAndroidSherpaLibs: calls.androidLibs,
}))
vi.mock("../lib/ios/sherpa.js", () => ({
  iosSherpaLibDir: slice => "/prepared/" + slice,
  ensureIosSherpaLibs: calls.iosLibs,
}))
vi.mock("node:fs", async importOriginal => {
  const fs = await importOriginal()
  return {
    ...fs,
    existsSync: file => file !== pagefindPath || calls.pagefindExists(),
    readFileSync: (file, ...args) => {
      if (file === pagefindPath) {
        return pagefindBytes
      }
      return file === pagefindReceiptPath ? calls.pagefindReceipt() : fs.readFileSync(file, ...args)
    },
  }
})
vi.mock("node:fs/promises", () => ({
  cp: vi.fn(),
  mkdir: vi.fn(),
  stat: async () => ({
    isDirectory: () => true,
    isFile: () => true,
  }),
}))

const platform = Object.getOwnPropertyDescriptor(process, "platform")
const argv = process.argv
beforeEach(() => {
  vi.stubEnv("SHERPA_ONNX_LIB_DIR", undefined)
  vi.stubEnv("SHERPA_REQUIRE_SOURCE", undefined)
  vi.resetModules()
  vi.clearAllMocks()
  calls.pagefindExists.mockReturnValue(true)
  calls.pagefindReceipt.mockReturnValue(JSON.stringify({
    tools: { filePrefix: PAGEFIND_WASM_PATH_PREFIX },
    sources: { pagefind: PAGEFIND_SOURCE_REVISION },
    artifacts: {
      pagefind: {
        bytes: pagefindBytes.length,
        sha256: createHash("sha256").update(pagefindBytes).digest("hex"),
      },
    },
  }))
  vi.stubEnv("PAPERCUT_DESKTOP_BUNDLES", "")
  vi.spyOn(console, "log").mockImplementation(() => {})
})
afterEach(() => {
  Object.defineProperty(process, "platform", platform)
  process.argv = argv
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
})

// Capture the actual entry scripts' native boundary without a toolchain, downloads or signing.
async function build(file, target, args = []) {
  Object.defineProperty(process, "platform", { value: target })
  process.argv = ["node", file, ...args]
  await import("../" + file)
}

it.each(["linux", "win32", "darwin"])("desktop ships the shared provider on %s without a new build path", async target => {
  const bundles = target === "linux" ? ["--bundles", "deb"] : []
  await build("desktop/build-desktop.js", target, bundles)
  const features = "native-tts-shared,experimental-search-embeddings"
  expect(calls.resources.mock.calls[0][0]).toBe(({
    linux: "linux",
    win32: "windows",
    darwin: "macos",
  })[target])
  expect(calls.resources.mock.calls[0][1].feature).toBe(features)
  expect(calls.runSync.mock.calls[0][1]).toEqual(["build", "--features", features, ...bundles])
  expect(calls.runSync.mock.calls[0][2].env.PAPERCUT_NATIVE_TTS_LINK).toBe("shared")
  expect(calls.runSync.mock.calls[0][2].env.PAPERCUT_BUILD_REVISION).toBe("cd".repeat(20))
})
it("keeps the explicit static desktop alternative separate", async () => {
  await build("desktop/build-desktop.js", "linux", ["--static"])
  expect(calls.runSync.mock.calls[0][1].slice(0, 3)).toEqual(["build", "--features", "native-tts-static"])
})
it("Android's shipping native APK includes the provider and its prepared runtime", async () => {
  await build("android/build-android.js", "linux", ["--native-tts"])
  expect(calls.androidLibs).toHaveBeenCalledOnce()
  expect(process.env.SHERPA_REQUIRE_SOURCE).toBe("1")
  expect(process.env.SHERPA_ONNX_LIB_DIR).toBe(join(ROOT, ".cache/android-production/source-arm64"))
  expect(calls.android).toHaveBeenCalledWith(["android", "build", "--apk", "--debug", "--target", "aarch64", "--features", "native-tts-shared,experimental-search-embeddings"], {
    SHERPA_ONNX_LIB_DIR: "/prepared/android",
    ORT_LIB_LOCATION: "/prepared/android",
  })
})
it("keeps the explicit no-native Android build and custom native arguments", async () => {
  await build("android/build-android.js", "linux")
  expect(calls.clearAndroidLibs).toHaveBeenCalledOnce()
  expect(calls.androidLibs).not.toHaveBeenCalled()
  expect(calls.android).toHaveBeenLastCalledWith(["android", "build", "--apk", "--debug"])
  vi.resetModules()
  const args = ["android", "build", "--apk", "--target", "aarch64", "--features", "native-tts-shared,experimental-search-embeddings"]
  await build("android/build-android.js", "linux", ["--native-tts", ...args])
  expect(calls.android.mock.calls.at(-1)[0]).toEqual(args)
})
it("production Android builds retain native TTS/search and require source PDF/OCR and Pagefind", async () => {
  vi.stubEnv("PAPERCUT_PDF_OCR_FROM_SOURCE", "0")
  vi.stubEnv("PAGEFIND_EXTENDED_BINARY_PATH", "/prebuilt/pagefind")
  await build("android/build-android.js", "linux", ["--native-tts", "--production"])
  expect(calls.android).toHaveBeenCalledWith(["android", "build", "--apk", "--target", "aarch64", "--features", "native-tts-shared,experimental-search-embeddings"], {
    RUSTUP_TOOLCHAIN: "1.98.1",
    PAPERCUT_BUILD_REVISION: "cd".repeat(20),
    CARGO_ENCODED_RUSTFLAGS: "production remaps",
    PAPERCUT_PDF_OCR_FROM_SOURCE: "1",
    PAGEFIND_EXTENDED_BINARY_PATH: join(ROOT, ".cache/android-production/source-pagefind/pagefind"),
    SHERPA_ONNX_LIB_DIR: "/prepared/android",
    ORT_LIB_LOCATION: "/prepared/android",
  })
})
it("rejects a cached Pagefind build from before WASM path normalization", async () => {
  calls.pagefindReceipt.mockReturnValueOnce(JSON.stringify({ tools: {} }))
  await expect(build("android/build-android.js", "linux", ["--native-tts", "--production"]))
    .rejects.toThrow("Missing Pagefind built with normalized WASM paths")
  expect(calls.android).not.toHaveBeenCalled()
})
it("rejects a missing Pagefind binary before launching the build", async () => {
  calls.pagefindExists.mockReturnValueOnce(false)
  await expect(build("android/build-android.js", "linux", ["--native-tts", "--production"]))
    .rejects.toThrow("Missing source-built Pagefind")
  expect(calls.android).not.toHaveBeenCalled()
})
it("rejects a stale Pagefind source pin or a mismatched binary", async () => {
  const receipt = JSON.parse(calls.pagefindReceipt())
  calls.pagefindReceipt.mockReturnValueOnce(JSON.stringify({
    ...receipt,
    sources: { pagefind: "old revision" },
  }))
  await expect(build("android/build-android.js", "linux", ["--native-tts", "--production"]))
    .rejects.toThrow("Pagefind source revision differs")
  vi.resetModules()
  receipt.artifacts.pagefind.sha256 = "ab".repeat(32)
  calls.pagefindReceipt.mockReturnValueOnce(JSON.stringify(receipt))
  await expect(build("android/build-android.js", "linux", ["--native-tts", "--production"]))
    .rejects.toThrow("Pagefind binary differs")
  expect(calls.android).not.toHaveBeenCalled()
})
it.each([
  ["--production"],
  ["--native-tts", "--production", "android", "build", "--debug"],
])("rejects production arguments that could drop shipping features: %j", async (...args) => {
  await expect(build("android/build-android.js", "linux", args)).rejects.toThrow("fixed arm64 shipping features")
  expect(calls.android).not.toHaveBeenCalled()
  expect(calls.androidLibs).not.toHaveBeenCalled()
})
it.each([{ args: [] }, { args: ["--ci-check"] }])("iOS native export/archive uses the linked provider with $args", async ({ args }) => {
  await build("ios/build-ios.js", "darwin", ["--native-tts", ...args])
  const call = calls.runSync.mock.calls[0]
  expect(call[2].env.PAPERCUT_BUILD_REVISION).toBe("cd".repeat(20))
  expect(call[1]).toContain("native-tts-ios,experimental-search-embeddings-linked")
  expect(call[1]).not.toContain("experimental-search-embeddings")
  expect(call[2].env.ORT_LIB_LOCATION).toEqual(call[2].env.SHERPA_ONNX_LIB_DIR)
  if (args.length) {
    expect(call[1]).toContain("--archive-only")
  } else {
    expect(call[1]).toContain("app-store-connect")
  }
})
it("unsigned iOS device validation checks the same linked provider and packaged protocol", async () => {
  await build("ios/check-ios-device-build.js", "darwin")
  const cargo = calls.runSync.mock.calls.find(([command]) => command === "cargo")
  expect(cargo[2].env.PAPERCUT_BUILD_REVISION).toBe("cd".repeat(20))
  expect(cargo[1][cargo[1].indexOf("--features") + 1]).toBe("native-tts-ios,experimental-search-embeddings-linked,tauri/custom-protocol")
  expect(calls.runSync.mock.calls.find(([command]) => command === "xcodebuild")[1]).toContain("CODE_SIGNING_ALLOWED=NO")
})
