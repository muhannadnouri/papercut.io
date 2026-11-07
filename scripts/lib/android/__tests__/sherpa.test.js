import { createHash } from "node:crypto"
import {
  mkdtemp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"
import {
  ANDROID_NDK_VERSION,
  ANDROID_ORT_AAR,
  SHERPA_ANDROID_SOURCE_REVISION,
} from "../constants.js"

const state = vi.hoisted(() => ({
  stage: "",
  download: vi.fn(),
}))
vi.mock("../../download.js", () => ({ downloadFile: state.download }))
vi.mock("../constants.js", async importOriginal => ({
  ...await importOriginal(),
  get SHERPA_GENERATED_ANDROID_JNI_LIBS() {
    return state.stage
  },
}))

let directory
beforeEach(async () => {
  vi.resetModules()
  vi.clearAllMocks()
  directory = await mkdtemp(join(tmpdir(), "papercut-android-libs-"))
  state.stage = join(directory, "stage")
  vi.stubEnv("SHERPA_ONNX_LIB_DIR", join(directory, "source"))
  await mkdir(process.env.SHERPA_ONNX_LIB_DIR)
  const artifacts = {}
  for (const lib of ["libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    await writeFile(join(process.env.SHERPA_ONNX_LIB_DIR, lib), lib)
    artifacts[lib] = { sha256: createHash("sha256").update(lib).digest("hex") }
  }
  await writeFile(join(process.env.SHERPA_ONNX_LIB_DIR, "source-receipt.json"), JSON.stringify({
    sources: {
      sherpa: SHERPA_ANDROID_SOURCE_REVISION,
      ortAar: ANDROID_ORT_AAR,
    },
    tools: {
      ndk: ANDROID_NDK_VERSION,
      api: 26,
    },
    artifacts,
  }))
})
afterEach(async () => {
  vi.unstubAllEnvs()
  await rm(directory, {
    recursive: true,
    force: true,
  })
})

it("stages source-provided libraries for arm64 without downloading, staging ORT, or keeping unused wrappers", async () => {
  const source = process.env.SHERPA_ONNX_LIB_DIR
  const target = join(state.stage, "arm64-v8a")
  await mkdir(target, { recursive: true })
  await writeFile(join(target, "libsherpa-onnx-jni.so"), "stale wrapper")
  await writeFile(join(target, "libonnxruntime.so"), "stale archive copy")
  await mkdir(join(state.stage, "x86_64"), { recursive: true })
  await writeFile(join(state.stage, "x86_64/libsherpa-onnx-c-api.so"), "stale archive ABI")
  const { androidSherpaLibDir, ensureAndroidSherpaLibs } = await import("../sherpa.js")
  expect(androidSherpaLibDir()).toBe(source)
  await ensureAndroidSherpaLibs({ force: true })
  expect(await readFile(join(target, "libsherpa-onnx-c-api.so"), "utf8")).toBe("libsherpa-onnx-c-api.so")
  // ORT ships from the Gradle Maven AAR; staging must not copy or keep a second copy.
  await expect(readFile(join(target, "libonnxruntime.so"))).rejects.toThrow()
  await expect(readFile(join(target, "libsherpa-onnx-jni.so"))).rejects.toThrow()
  await expect(readFile(join(state.stage, "x86_64/libsherpa-onnx-c-api.so"))).rejects.toThrow()
  expect(state.download).not.toHaveBeenCalled()
})

it("rejects missing/empty external libraries without silently falling back to prebuilts", async () => {
  await rm(join(process.env.SHERPA_ONNX_LIB_DIR, "libsherpa-onnx-c-api.so"))
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("refusing a prebuilt fallback")
  await writeFile(join(process.env.SHERPA_ONNX_LIB_DIR, "libsherpa-onnx-c-api.so"), "")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("refusing a prebuilt fallback")
  expect(state.download).not.toHaveBeenCalled()
})

it("rejects a runtime built with the previous NDK", async () => {
  const path = join(process.env.SHERPA_ONNX_LIB_DIR, "source-receipt.json")
  const receipt = JSON.parse(await readFile(path, "utf8"))
  receipt.tools.ndk = "29.0.13846066"
  await writeFile(path, JSON.stringify(receipt))
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("Rebuild the Android source libraries")
})

it.each([
  ["sherpa", "old revision", "pinned Sherpa revision"],
  ["ortAar", {
    ...ANDROID_ORT_AAR,
    sha256: "ab".repeat(32),
  }, "pinned ONNX Runtime AAR"],
])("rejects a stale %s source pin even when the artifacts match", async (name, pin, message) => {
  const path = join(process.env.SHERPA_ONNX_LIB_DIR, "source-receipt.json")
  const receipt = JSON.parse(await readFile(path, "utf8"))
  receipt.sources[name] = pin
  await writeFile(path, JSON.stringify(receipt))
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow(message)
  expect(state.download).not.toHaveBeenCalled()
})

it("clears stale TTS inputs for a no-native APK without touching other JNI files", async () => {
  const target = join(state.stage, "arm64-v8a")
  await mkdir(target, { recursive: true })
  await writeFile(join(target, "libsherpa-onnx-c-api.so"), "stale Sherpa")
  await writeFile(join(target, "libonnxruntime.so"), "stale ORT")
  await writeFile(join(target, "libother.so"), "keep")
  const { clearAndroidSherpaLibs } = await import("../sherpa.js")
  await clearAndroidSherpaLibs()
  await expect(readFile(join(target, "libsherpa-onnx-c-api.so"))).rejects.toThrow()
  await expect(readFile(join(target, "libonnxruntime.so"))).rejects.toThrow()
  expect(await readFile(join(target, "libother.so"), "utf8")).toBe("keep")
})

it.each(["libsherpa-onnx-c-api.so", "libonnxruntime.so"])("rejects a changed %s instead of trusting a stale receipt", async lib => {
  await writeFile(join(process.env.SHERPA_ONNX_LIB_DIR, lib), "changed")
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("differs from its receipt")
})

it("refuses the prebuilt archive when source-only mode has no source directory", async () => {
  vi.stubEnv("SHERPA_ONNX_LIB_DIR", undefined)
  vi.stubEnv("SHERPA_REQUIRE_SOURCE", "1")
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("SHERPA_REQUIRE_SOURCE is set")
  expect(state.download).not.toHaveBeenCalled()
})
