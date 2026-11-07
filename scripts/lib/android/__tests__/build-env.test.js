import { join } from "node:path"
import { readFileSync } from "node:fs"
import {
  mkdtemp,
  mkdir,
  rm,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"
import { ROOT } from "../../paths.js"
import {
  androidBuildPathMaps,
  androidBuildEnv,
  androidNdkDir,
  androidProductionEnv,
} from "../build-env.js"
import { ANDROID_NDK_VERSION } from "../constants.js"

const compiler = vi.hoisted(() => vi.fn())
vi.mock("node:child_process", () => ({ execFileSync: compiler }))
vi.mock("../../build-revision.js", () => ({ productionBuildRevision: () => "cd".repeat(20) }))
vi.mock("../jdk.js", () => ({ ensureLocalJdk: async () => "/jdk" }))

let sdk
async function ndkAt(directory, revision = ANDROID_NDK_VERSION) {
  await mkdir(directory, { recursive: true })
  await writeFile(join(directory, "source.properties"), "Pkg.Revision = " + revision + "\n")
  return directory
}

beforeEach(async () => {
  sdk = await mkdtemp(join(tmpdir(), "papercut-ndk-"))
  await ndkAt(join(sdk, "ndk", ANDROID_NDK_VERSION))
  compiler.mockImplementation((_file, args) => args[0] === "-vV"
    ? "release: 1.98.1\ncommit-hash: " + "ab".repeat(20) + "\n"
    : "/toolchains/1.98.1\n")
  vi.stubEnv("ANDROID_HOME", sdk)
  vi.stubEnv("NDK_HOME", undefined)
  vi.stubEnv("ANDROID_NDK_HOME", undefined)
  vi.stubEnv("ANDROID_NDK", undefined)
  vi.stubEnv("CARGO_HOME", join(ROOT, "cache with spaces/cargo"))
  vi.stubEnv("RUSTUP_HOME", "/toolchains")
  vi.stubEnv("RUSTFLAGS", "-C opt-level=2")
  vi.stubEnv("CARGO_ENCODED_RUSTFLAGS", undefined)
})
afterEach(async () => {
  vi.unstubAllEnvs()
  await rm(sdk, {
    recursive: true,
    force: true,
  })
})

it("pins production Rust, preserves flags and keeps nested paths/spaces intact", () => {
  const env = androidProductionEnv()
  const flags = env.CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  expect(env.RUSTUP_TOOLCHAIN).toBe("1.98.1")
  expect(env.PAPERCUT_BUILD_REVISION).toBe("cd".repeat(20))
  expect(flags.slice(0, 2)).toEqual(["-C", "opt-level=2"])
  expect(flags).toContain("-Clink-arg=-landroid")
  expect(flags).toContain("-Clink-arg=-llog")
  expect(flags).toContain("-Clink-arg=-lOpenSLES")
  const workspace = flags.indexOf(`--remap-path-prefix=${ROOT}=/build/papercut`)
  const cargo = flags.indexOf(`--remap-path-prefix=${process.env.CARGO_HOME}=/cargo`)
  expect(workspace).toBeGreaterThan(-1)
  expect(cargo).toBeGreaterThan(workspace)
  expect(flags).toContain("--remap-path-prefix=/toolchains=/rustup")
  expect(flags).toContain("--remap-path-prefix=/toolchains/1.98.1/lib/rustlib/src/rust=/rustc/" + "ab".repeat(20))
  expect(flags).toContain(`--remap-path-prefix=${join(sdk, "ndk", ANDROID_NDK_VERSION)}=/android-ndk`)
})

it("honors encoded flag precedence and an explicit NDK directory", async () => {
  vi.stubEnv("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1fflag_with_spaces=\"a b\"")
  const directory = await ndkAt(join(sdk, "custom ndk"))
  vi.stubEnv("NDK_HOME", directory)
  const flags = androidProductionEnv().CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  expect(flags.slice(0, 2)).toEqual(["--cfg", "flag_with_spaces=\"a b\""])
  expect(flags).not.toContain("opt-level=2")
  expect(androidBuildPathMaps()).toContainEqual([directory, "/android-ndk"])
})

it.each(["29.0.13846066", ANDROID_NDK_VERSION + "-beta1", ANDROID_NDK_VERSION + "0"])("rejects the wrong or preview NDK %s", async revision => {
  await ndkAt(join(sdk, "ndk", ANDROID_NDK_VERSION), revision)
  expect(androidNdkDir).toThrow("Android builds require NDK " + ANDROID_NDK_VERSION)
})

it("honors the F-Droid NDK path and normalizes all selectors for every Android build", async () => {
  const directory = await ndkAt(join(sdk, "fdroid ndk"))
  vi.stubEnv("ANDROID_NDK_HOME", directory)
  expect(androidNdkDir()).toBe(directory)
  const env = await androidBuildEnv({ SHERPA_ONNX_LIB_DIR: "/runtime" })
  for (const key of ["NDK_HOME", "ANDROID_NDK_HOME", "ANDROID_NDK"]) {
    expect(env[key]).toBe(directory)
  }
  expect(env.SHERPA_ONNX_LIB_DIR).toBe("/runtime")
  expect(env.JAVA_HOME).toBe("/jdk")
})

it("validates caller overrides before launching Android tools", async () => {
  const directory = await ndkAt(join(sdk, "stale ndk"), "29.0.13846066")
  await expect(androidBuildEnv({ NDK_HOME: directory })).rejects.toThrow("Android builds require NDK")
})

it("keeps the Gradle and F-Droid declarations aligned with the shared pin", () => {
  const gradle = readFileSync(join(ROOT, "src-tauri/gen/android/app/build.gradle.kts"), "utf8")
  const metadata = readFileSync(join(ROOT, "docs/fdroid/metadata/io.papercut.desktop.yml"), "utf8")
  expect(gradle.match(/\bndkVersion\s*=\s*"([^"]+)"/)[1]).toBe(ANDROID_NDK_VERSION)
  expect(metadata.match(/^\s+ndk:\s*(\S+)/m)[1]).toBe(ANDROID_NDK_VERSION)
})

it("fails closed without an Android SDK", () => {
  vi.stubEnv("ANDROID_HOME", undefined)
  vi.stubEnv("ANDROID_SDK_ROOT", undefined)
  expect(androidProductionEnv).toThrow("Set ANDROID_HOME or ANDROID_SDK_ROOT")
})

it("rejects a compiler override that does not match the pinned Rust release", () => {
  compiler.mockReturnValue("release: 1.99.0\n")
  expect(androidProductionEnv).toThrow("require the pinned Rust toolchain")
})
