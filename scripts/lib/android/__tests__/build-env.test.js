import { join } from "node:path"
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
  androidProductionEnv,
} from "../build-env.js"

const compiler = vi.hoisted(() => vi.fn())
vi.mock("node:child_process", () => ({ execFileSync: compiler }))

beforeEach(() => {
  compiler.mockImplementation((_file, args) => args[0] === "-vV"
    ? "release: 1.98.1\ncommit-hash: " + "ab".repeat(20) + "\n"
    : "/toolchains/1.98.1\n")
  vi.stubEnv("ANDROID_HOME", "/sdk")
  vi.stubEnv("NDK_HOME", undefined)
  vi.stubEnv("CARGO_HOME", join(ROOT, "cache with spaces/cargo"))
  vi.stubEnv("RUSTUP_HOME", "/toolchains")
  vi.stubEnv("RUSTFLAGS", "-C opt-level=2")
  vi.stubEnv("CARGO_ENCODED_RUSTFLAGS", undefined)
})
afterEach(() => vi.unstubAllEnvs())

it("pins production Rust, preserves flags and keeps nested paths/spaces intact", () => {
  const env = androidProductionEnv()
  const flags = env.CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  expect(env.RUSTUP_TOOLCHAIN).toBe("1.98.1")
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
  expect(flags).toContain("--remap-path-prefix=/sdk/ndk/29.0.13846066=/android-ndk")
})

it("honors encoded flag precedence and an explicit NDK directory", () => {
  vi.stubEnv("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1fflag_with_spaces=\"a b\"")
  vi.stubEnv("NDK_HOME", "/custom ndk")
  const flags = androidProductionEnv().CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  expect(flags.slice(0, 2)).toEqual(["--cfg", "flag_with_spaces=\"a b\""])
  expect(flags).not.toContain("opt-level=2")
  expect(androidBuildPathMaps()).toContainEqual(["/custom ndk", "/android-ndk"])
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
