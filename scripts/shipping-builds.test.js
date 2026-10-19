import { afterEach, beforeEach, expect, it, vi } from "vitest"

const calls = vi.hoisted(() => ({
  runSync: vi.fn(() => ({ status: 0 })),
  exitFromResult: vi.fn(),
  android: vi.fn(),
  androidLibs: vi.fn(),
  iosLibs: vi.fn(),
  prepare: vi.fn(),
  resources: vi.fn(),
}))
vi.mock("./lib/process.js", () => ({ runSync: calls.runSync, exitFromResult: calls.exitFromResult }))
vi.mock("./lib/tauri.js", () => ({ tauriCommand: args => ({ command: "tauri", args }) }))
vi.mock("./lib/desktop/platform.js", () => ({
  currentDesktopPlatform: () => ({ linux: "linux", win32: "windows", darwin: "macos" })[process.platform],
  prepareDesktopBuild: calls.prepare,
  desktopBuildEnv: (_platform, env) => env,
  prepareDesktopBundleResources: calls.resources,
}))
vi.mock("./lib/android/build-env.js", () => ({ runTauriAndroidBuild: calls.android }))
vi.mock("./lib/android/sherpa.js", () => ({ androidSherpaLibDir: () => "/prepared/android", ensureAndroidSherpaLibs: calls.androidLibs }))
vi.mock("./lib/ios/sherpa.js", () => ({ iosSherpaLibDir: slice => "/prepared/" + slice, ensureIosSherpaLibs: calls.iosLibs }))
vi.mock("node:fs", async importOriginal => ({ ...await importOriginal(), existsSync: () => true }))
vi.mock("node:fs/promises", () => ({ cp: vi.fn(), mkdir: vi.fn(), stat: async () => ({ isDirectory: () => true, isFile: () => true }) }))

const platform = Object.getOwnPropertyDescriptor(process, "platform")
const argv = process.argv
beforeEach(() => {
  vi.resetModules()
  vi.clearAllMocks()
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
  await import("./" + file)
}

it.each(["linux", "win32", "darwin"])("desktop ships the shared provider on %s without a new build path", async target => {
  const bundles = target === "linux" ? ["--bundles", "deb"] : []
  await build("build-desktop.js", target, bundles)
  const features = "native-tts-shared,experimental-search-embeddings,native-translation-ctranslate2,native-translation-llama"
  expect(calls.resources.mock.calls[0][0]).toBe(({ linux: "linux", win32: "windows", darwin: "macos" })[target])
  expect(calls.resources.mock.calls[0][1].feature).toBe(features)
  expect(calls.runSync.mock.calls[0][1]).toEqual(["build", "--features", features, ...bundles])
  expect(calls.runSync.mock.calls[0][2].env.PAPERCUT_NATIVE_TTS_LINK).toBe("shared")
})
it("keeps the explicit static desktop alternative separate", async () => {
  await build("build-desktop.js", "linux", ["--static"])
  expect(calls.runSync.mock.calls[0][1].slice(0, 3)).toEqual(["build", "--features", "native-tts-static,native-translation-ctranslate2,native-translation-llama"])
})
it("disabling translation retains shared TTS and conceptual search", async () => {
  await build("build-desktop.js", "linux", ["--no-translation"])
  const features = "native-tts-shared,experimental-search-embeddings"
  expect(calls.resources.mock.calls[0][1].feature).toBe(features)
  expect(calls.runSync.mock.calls[0][1]).toEqual(["build", "--features", features])
})
it("Android's shipping native APK includes the provider and its prepared runtime", async () => {
  await build("build-android.js", "linux", ["--native-tts"])
  expect(calls.androidLibs).toHaveBeenCalledOnce()
  expect(calls.android).toHaveBeenCalledWith(["android", "build", "--apk", "--debug", "--target", "aarch64", "--features", "native-tts-shared,experimental-search-embeddings"], { SHERPA_ONNX_LIB_DIR: "/prepared/android", ORT_LIB_LOCATION: "/prepared/android" })
})
it("keeps the explicit no-native Android build and custom native arguments", async () => {
  await build("build-android.js", "linux")
  expect(calls.android).toHaveBeenLastCalledWith(["android", "build", "--apk", "--debug"])
  vi.resetModules()
  const args = ["android", "build", "--apk", "--release", "--target", "aarch64", "--features", "native-tts-shared,experimental-search-embeddings"]
  await build("build-android.js", "linux", ["--native-tts", ...args])
  expect(calls.android.mock.calls.at(-1)[0]).toEqual(args)
})
it.each([{ args: [] }, { args: ["--ci-check"] }])("iOS native export/archive uses the linked provider with $args", async ({ args }) => {
  await build("build-ios.js", "darwin", ["--native-tts", ...args])
  const call = calls.runSync.mock.calls[0]
  expect(call[1]).toContain("native-tts-ios,experimental-search-embeddings-linked")
  expect(call[1]).not.toContain("experimental-search-embeddings")
  expect(call[2].env.ORT_LIB_LOCATION).toEqual(call[2].env.SHERPA_ONNX_LIB_DIR)
  if (args.length) expect(call[1]).toContain("--archive-only")
  else expect(call[1]).toContain("app-store-connect")
})
it("unsigned iOS device validation checks the same linked provider and packaged protocol", async () => {
  await build("check-ios-device-build.js", "darwin")
  const cargo = calls.runSync.mock.calls.find(([command]) => command === "cargo")
  expect(cargo[1][cargo[1].indexOf("--features") + 1]).toBe("native-tts-ios,experimental-search-embeddings-linked,tauri/custom-protocol")
  expect(calls.runSync.mock.calls.find(([command]) => command === "xcodebuild")[1]).toContain("CODE_SIGNING_ALLOWED=NO")
})

// A worker opt-in adds only the base host and leaves existing SDK/TTS choices alone.
it.each([false, true])("Android worker opt-in composes with native TTS=%s", async nativeTts => {
  await build("build-android.js", "linux", [...(nativeTts ? ["--native-tts"] : []), "--translation-worker"])
  const args = calls.android.mock.calls[0][0]
  expect(args.slice(0, 4)).toEqual(["android", "build", "--features", "native-translation"])
  expect(args).not.toContain("--translation-worker")
  expect(args.join(" ")).not.toMatch(/native-translation-(ctranslate2|llama)/)
  expect(args).toContain("--apk")
  expect(calls.androidLibs).toHaveBeenCalledTimes(nativeTts ? 1 : 0)
  if (nativeTts) {
    expect(args).toContain("native-tts-shared,experimental-search-embeddings")
    expect(calls.android.mock.calls[0][1].ORT_LIB_LOCATION).toBe("/prepared/android")
  }
})
it("Android worker opt-in retains custom features, target and runner arguments", async () => {
  const args = ["android", "build", "--apk", "--target", "x86_64", "--features=experimental-search-embeddings", "--", "--stacktrace"]
  await build("build-android.js", "linux", ["--translation-worker", ...args])
  expect(calls.android.mock.calls[0][0]).toEqual(["android", "build", "--features", "native-translation", ...args.slice(2)])
  expect(calls.androidLibs).not.toHaveBeenCalled()
})
it("Android rejects the worker build flag on initialization before invoking SDK tools", async () => {
  await expect(build("build-android.js", "linux", ["--translation-worker", "android", "init"])).rejects.toThrow("requires an Android build")
  expect(calls.android).not.toHaveBeenCalled()
  expect(calls.androidLibs).not.toHaveBeenCalled()
})
it.each([false, true])("iOS worker-only export/archive needs no Sherpa setup (ci=%s)", async ci => {
  await build("build-ios.js", "darwin", ["--translation-worker", ...(ci ? ["--ci-check"] : [])])
  const args = calls.runSync.mock.calls[0][1]
  expect(args).toContain("native-translation")
  expect(args).not.toContain("--translation-worker")
  expect(args.join(" ")).not.toMatch(/native-translation-(ctranslate2|llama)|native-tts/)
  expect(calls.iosLibs).not.toHaveBeenCalled()
  if (ci) expect(args).toContain("--archive-only")
  else expect(args).toContain("app-store-connect")
})
it.each([{ args: [] }, { args: ["--ci-check"] }])("iOS native TTS and worker opt-in share one feature list with $args", async ({ args }) => {
  await build("build-ios.js", "darwin", ["--native-tts", "--translation-worker", ...args])
  const call = calls.runSync.mock.calls[0]
  expect(call[1]).toContain("native-tts-ios,experimental-search-embeddings-linked,native-translation")
  expect(call[1].filter(arg => arg === "--features")).toHaveLength(1)
  expect(call[2].env.ORT_LIB_LOCATION).toEqual(call[2].env.SHERPA_ONNX_LIB_DIR)
  expect(calls.iosLibs).toHaveBeenCalledOnce()
})
it("iOS worker opt-in preserves custom target, features and export arguments", async () => {
  await build("build-ios.js", "darwin", ["--translation-worker", "--target", "aarch64", "--features=tauri/custom-protocol", "--export-method", "ad-hoc"])
  expect(calls.runSync.mock.calls[0][1]).toEqual(["ios", "build", "--features", "native-translation", "--target", "aarch64", "--features=tauri/custom-protocol", "--export-method", "ad-hoc"])
})
it("iOS rejects worker opt-in during project initialization", async () => {
  vi.spyOn(process, "exit").mockImplementation(() => { throw new Error("invalid build flags") })
  vi.spyOn(console, "error").mockImplementation(() => {})
  await expect(build("build-ios.js", "darwin", ["--translation-worker", "--init"])).rejects.toThrow("invalid build flags")
  expect(console.error).toHaveBeenCalledWith(expect.stringContaining("cannot be used with --init"))
  expect(calls.runSync).not.toHaveBeenCalled()
  expect(calls.iosLibs).not.toHaveBeenCalled()
})
it("iOS unsigned device opt-in retains the deployment target, provider and custom features", async () => {
  await build("check-ios-device-build.js", "darwin", ["--translation-worker", "--features", "extra-test-feature"])
  const cargo = calls.runSync.mock.calls.find(([command]) => command === "cargo")
  const features = cargo[1][cargo[1].indexOf("--features") + 1]
  expect(features).toBe("native-tts-ios,experimental-search-embeddings-linked,tauri/custom-protocol,native-translation,extra-test-feature")
  expect(cargo[2].env.IPHONEOS_DEPLOYMENT_TARGET).toBeTruthy()
  expect(features).not.toMatch(/native-translation-(ctranslate2|llama)/)
  expect(calls.runSync.mock.calls.find(([command]) => command === "xcodebuild")[1]).toContain("CODE_SIGNING_ALLOWED=NO")
})
it.each([false, true])("Android keeps a runner's worker-named argument opaque (opt-in=%s)", async enabled => {
  const args = ["android", "build", "--apk", "--", "--translation-worker"]
  await build("build-android.js", "linux", [...(enabled ? ["--translation-worker"] : []), ...args])
  expect(calls.android.mock.calls[0][0]).toEqual(enabled ? ["android", "build", "--features", "native-translation", ...args.slice(2)] : args)
})
it.each([false, true])("iOS keeps runner flags/targets opaque (opt-in=%s)", async enabled => {
  const runner = ["--", "--translation-worker", "--target", "xcode-only-target"]
  await build("build-ios.js", "darwin", ["--ci-check", ...(enabled ? ["--translation-worker"] : []), ...runner])
  expect(calls.runSync.mock.calls[0][1]).toEqual(["ios", "build", "--target", "aarch64-sim", "--archive-only", ...(enabled ? ["--features", "native-translation"] : []), ...runner])
})
