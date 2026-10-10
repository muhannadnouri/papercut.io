import { spawnSync } from "node:child_process"
import { chmodSync, copyFileSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { randomUUID } from "node:crypto"
import { parseArgs } from "node:util"
import { isMain, shQuote } from "./lib/process.js"

const PROVIDER_TEST = "search_embeddings::provider::tests::cached_provider_activation_switch_failure_and_unload"
const TEST_ARGS = [PROVIDER_TEST, "--ignored", "--exact", "--nocapture"]

// Cargo's JSON output avoids guessing which hashed executable belongs to this build.
export function testExecutable(buildLog) {
  const artifacts = buildLog.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line))
    .filter((item) => item.reason === "compiler-artifact" && item.target?.name === "app_lib"
      && item.profile?.test && item.executable)
  if (artifacts.length !== 1) throw new Error("Expected exactly one app_lib test executable")
  return resolve(artifacts[0].executable)
}

// Exit zero alone can hide an empty filter or an inherited single-format override.
export function assertInferenceCompleted(output) {
  for (const format of ["Float32", "DynamicInt8"]) {
    if (!output.includes(`lifecycle cycle complete: 1/1 ${format}`)) {
      throw new Error(`Missing completed ${format} lifecycle`)
    }
  }
  if (!output.includes("test result: ok. 1 passed; 0 failed; 0 ignored;")) {
    throw new Error("The exact ignored provider test did not pass")
  }
}

// Bound every native operation; capture failures without hiding the loader/test diagnostics.
export function command(program, args, { timeout = 120_000, env = process.env, combined = false } = {}) {
  const result = spawnSync(program, args, { env, timeout, killSignal: "SIGKILL", encoding: "utf8", maxBuffer: 16 * 1024 * 1024 })
  if (result.stdout) process.stdout.write(result.stdout)
  if (result.stderr) process.stderr.write(result.stderr)
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`${program} exited with ${result.status ?? result.signal}`)
  return (result.stdout ?? "") + (combined ? result.stderr ?? "" : "")
}

// Own only a disposable directory on the action's emulator, never a connected physical device.
export function checkAndroid({ executable, models, runtimeLibs }, run = command) {
  const port = process.env.EMULATOR_PORT || "5554"
  if (!/^\d+$/.test(port)) throw new Error("Invalid emulator port")
  const adb = (args, options) => run("adb", ["-s", `emulator-${port}`, ...args], options)
  if (adb(["shell", "getprop", "ro.product.cpu.abi"]).trim() !== "x86_64") {
    throw new Error("Provider validation requires an x86_64 emulator")
  }
  const remote = `/data/local/tmp/papercut-provider-${randomUUID()}`
  try {
    adb(["shell", "mkdir", "-p", remote])
    adb(["push", executable, `${remote}/provider-test`])
    adb(["push", runtimeLibs, `${remote}/libs`])
    adb(["push", models, `${remote}/models`])
    adb(["shell", "chmod", "700", `${remote}/provider-test`])
    // Remote timeout also bounds the native child if the host's adb process times out.
    const output = adb(["shell", [
      "unset PAPERCUT_EMBEDDING_TEST_FORMAT;",
      `LD_LIBRARY_PATH=${shQuote(`${remote}/libs`)}`,
      `PAPERCUT_EMBEDDING_TEST_MODELS=${shQuote(`${remote}/models`)}`,
      "PAPERCUT_EMBEDDING_TEST_DOWNLOAD=0 PAPERCUT_EMBEDDING_TEST_CYCLES=1",
      "timeout 600", shQuote(`${remote}/provider-test`), ...TEST_ARGS.map(shQuote),
    ].join(" ")], { timeout: 660_000, combined: true })
    assertInferenceCompleted(output)
  } finally {
    adb(["shell", "rm", "-rf", remote])
  }
}

// Pick a supported iPhone/runtime pair from Xcode rather than pinning runner device names.
export function simulatorDevice(devices) {
  for (const [runtime, entries] of Object.entries(devices)) {
    if (!runtime.includes(".iOS-")) continue
    const device = entries.find((item) => item.isAvailable
      && item.deviceTypeIdentifier?.includes(".iPhone-"))
    if (device) return { runtime, type: device.deviceTypeIdentifier }
  }
  throw new Error("No available iPhone simulator runtime")
}

// A private simulator and signed test copy keep shutdown/deletion away from other sessions.
export function checkIos({ executable, models }, run = command) {
  const { devices } = JSON.parse(run("xcrun", ["simctl", "list", "devices", "available", "--json"]))
  const selected = simulatorDevice(devices)
  const uuid = run("xcrun", ["simctl", "create", `Papercut-provider-${randomUUID()}`, selected.type, selected.runtime]).trim()
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(uuid)) {
    throw new Error("simctl did not return a simulator UUID")
  }
  let dir
  let booted = false
  try {
    dir = mkdtempSync(join(tmpdir(), "papercut-provider-"))
    const signedTest = join(dir, "provider-test")
    copyFileSync(executable, signedTest)
    chmodSync(signedTest, 0o700)
    run("codesign", ["--force", "--sign", "-", signedTest])
    run("xcrun", ["simctl", "boot", uuid])
    booted = true
    run("xcrun", ["simctl", "bootstatus", uuid, "-b"], { timeout: 300_000 })
    const env = { ...process.env }
    for (const key of Object.keys(env)) {
      if (/^(SIMCTL_CHILD_)?PAPERCUT_EMBEDDING_TEST_/.test(key)) delete env[key]
    }
    // simctl forwards only prefixed variables; the simulator can read host temporary assets.
    env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_MODELS = models
    env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_DOWNLOAD = "0"
    env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_CYCLES = "1"
    const output = run("xcrun", ["simctl", "spawn", uuid, signedTest, ...TEST_ARGS], { timeout: 660_000, env, combined: true })
    assertInferenceCompleted(output)
  } finally {
    try {
      // Shutdown stops any native child after a timeout before deleting the owned simulator.
      try {
        if (booted) run("xcrun", ["simctl", "shutdown", uuid])
      } finally {
        run("xcrun", ["simctl", "delete", uuid])
      }
    } finally {
      if (dir) rmSync(dir, { recursive: true, force: true })
    }
  }
}

if (isMain(import.meta.url)) {
  try {
    const { values } = parseArgs({ options: {
      platform: { type: "string" }, "build-log": { type: "string" },
      models: { type: "string" }, "runtime-libs": { type: "string" },
    } })
    if (!["android", "ios"].includes(values.platform) || !values["build-log"] || !values.models
      || (values.platform === "android" && !values["runtime-libs"])) {
      throw new Error("Supply --platform android|ios --build-log FILE --models DIR (Android: --runtime-libs DIR)")
    }
    const executable = testExecutable(readFileSync(values["build-log"], "utf8"))
    if (!statSync(executable).isFile()) throw new Error("Cargo test executable is not a file")
    const models = resolve(values.models)
    for (const format of ["float32", "dynamic-int8"]) {
      if (!statSync(join(models, format)).isDirectory()) throw new Error(`Missing ${format} assets`)
    }
    if (values.platform === "android") {
      const runtimeLibs = resolve(values["runtime-libs"])
      if (!statSync(runtimeLibs).isDirectory()) throw new Error("Missing Android runtime libraries")
      checkAndroid({ executable, models, runtimeLibs })
    } else {
      checkIos({ executable, models })
    }
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
