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

const state = vi.hoisted(() => ({
  stage: "",
  download: vi.fn(),
}))
vi.mock("../../download.js", () => ({ downloadFile: state.download }))
vi.mock("../constants.js", async importOriginal => ({
  ...await importOriginal(),
  SHERPA_GENERATED_ANDROID_JNI_LIBS: state.stage,
}))

let directory
beforeEach(async () => {
  vi.resetModules()
  vi.clearAllMocks()
  directory = await mkdtemp(join(tmpdir(), "papercut-android-libs-"))
  state.stage = join(directory, "stage")
  vi.stubEnv("SHERPA_ONNX_LIB_DIR", join(directory, "source"))
  await mkdir(process.env.SHERPA_ONNX_LIB_DIR)
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
  for (const lib of ["libsherpa-onnx-c-api.so", "libonnxruntime.so"]) {
    await writeFile(join(source, lib), lib)
  }
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
  const { ensureAndroidSherpaLibs } = await import("../sherpa.js")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("refusing a prebuilt fallback")
  await writeFile(join(process.env.SHERPA_ONNX_LIB_DIR, "libsherpa-onnx-c-api.so"), "")
  await expect(ensureAndroidSherpaLibs()).rejects.toThrow("refusing a prebuilt fallback")
  expect(state.download).not.toHaveBeenCalled()
})
