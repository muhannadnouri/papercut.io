import assert from "node:assert/strict"
import test from "node:test"
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { assertInferenceCompleted, checkAndroid, checkIos, command, simulatorDevice, testExecutable } from "./check-mobile-provider-inference.mjs"

const completed = [
  "lifecycle cycle complete: 1/1 Float32",
  "lifecycle cycle complete: 1/1 DynamicInt8",
  "test result: ok. 1 passed; 0 failed; 0 ignored;",
].join("\n")

test("native command wrapper preserves stderr markers and fails on exit errors and hangs", () => {
  assert.equal(command(process.execPath, ["-e", "process.stderr.write('marker')"], { combined: true }), "marker")
  assert.equal(command(process.execPath, ["-e", "process.stderr.write('diagnostic')"]), "")
  assert.throws(() => command(process.execPath, ["-e", "process.exit(3)"]), /exited with 3/)
  assert.throws(() => command(process.execPath, ["-e", "setInterval(() => {}, 1000)"], { timeout: 100 }), { code: "ETIMEDOUT" })
})

test("Cargo artifact selection and completion checks reject ambiguous builds and partial runs", () => {
  const artifact = { reason: "compiler-artifact", target: { name: "app_lib" }, profile: { test: true }, executable: "a test" }
  const log = JSON.stringify(artifact)
  assert.equal(testExecutable(log), resolve("a test"))
  for (const invalid of ["", "not JSON", `${log}\n${log}`, JSON.stringify({ ...artifact, profile: { test: false } })]) {
    assert.throws(() => testExecutable(invalid))
  }
  assertInferenceCompleted(completed)
  for (const invalid of ["", completed.replace("DynamicInt8", "Float32"), completed.replace("1 passed", "0 passed"), completed.replace("0 failed", "1 failed")]) {
    assert.throws(() => assertInferenceCompleted(invalid))
  }
})

test("Android never targets a physical device and removes only owned data on success, staging failure or timeout", () => {
  for (const failure of [null, "push", "inference"]) {
    const calls = []
    const run = (program, args, options) => {
      assert.equal(program, "adb")
      assert.equal(args[0], "-s")
      assert.match(args[1], /^emulator-\d+$/)
      calls.push({ args, options })
      if (args.includes("getprop")) return "x86_64\n"
      if (failure === "push" && args.includes("push")) throw new Error("staging failed")
      if (args[3]?.includes("timeout 600")) {
        if (failure === "inference") throw new Error("ETIMEDOUT")
        return completed
      }
      return ""
    }
    const check = () => checkAndroid({ executable: "/tmp/test' file", models: "/tmp/models", runtimeLibs: "/tmp/libs" }, run)
    if (failure) assert.throws(check, /staging failed|ETIMEDOUT/)
    else check()
    const remote = calls.find(({ args }) => args.includes("mkdir")).args.at(-1)
    assert.match(remote, /^\/data\/local\/tmp\/papercut-provider-[0-9a-f-]{36}$/)
    assert.deepEqual(calls.at(-1).args.slice(2), ["shell", "rm", "-rf", remote])
    if (!failure) {
      const inference = calls.find(({ options }) => options?.timeout === 660_000)
      assert.ok(inference.args[3].includes("PAPERCUT_EMBEDDING_TEST_DOWNLOAD=0"))
      assert.ok(inference.args[3].includes("unset PAPERCUT_EMBEDDING_TEST_FORMAT"))
      assert.ok(inference.args[3].includes("--ignored"))
      assert.ok(inference.args[3].includes("--exact"))
    }
  }
  assert.throws(() => checkAndroid({}, () => "arm64-v8a"), /x86_64 emulator/)
})

test("simulator selection skips unavailable devices and other Apple platforms", () => {
  const device = { isAvailable: true, deviceTypeIdentifier: "com.apple.CoreSimulator.SimDeviceType.iPhone-17" }
  const runtime = "com.apple.CoreSimulator.SimRuntime.iOS-26-2"
  assert.deepEqual(simulatorDevice({ "com.apple.CoreSimulator.SimRuntime.tvOS-26-2": [device], [runtime]: [{ ...device, isAvailable: false }, device] }), { runtime, type: device.deviceTypeIdentifier })
  assert.throws(() => simulatorDevice({ [runtime]: [{ ...device, isAvailable: false }] }), /No available/)
})

test("iOS isolates forwarded settings and cleans its simulator/test copy after success, signing failure or timeout", () => {
  const fixture = mkdtempSync(join(tmpdir(), "papercut-provider-selftest-"))
  const executable = join(fixture, "test")
  writeFileSync(executable, "fixture")
  const uuid = "12345678-1234-1234-1234-123456789abc"
  const envKey = "SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_FORMAT"
  const original = process.env[envKey]
  process.env[envKey] = "float32"
  try {
    for (const failure of [null, "codesign", "inference"]) {
      const calls = []
      let signedTest
      const run = (program, args, options) => {
        calls.push({ program, args, options })
        if (args.includes("list")) return JSON.stringify({ devices: {
          "com.apple.CoreSimulator.SimRuntime.iOS-26-2": [{ isAvailable: true, deviceTypeIdentifier: "com.apple.CoreSimulator.SimDeviceType.iPhone-17" }],
        } })
        if (args.includes("create")) return uuid
        if (program === "codesign") {
          signedTest = args.at(-1)
          assert.ok(existsSync(signedTest))
          if (failure === "codesign") throw new Error("sign failed")
        }
        if (args.includes("spawn")) {
          assert.equal(options.env[envKey], undefined)
          assert.equal(options.env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_DOWNLOAD, "0")
          assert.equal(options.env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_MODELS, "/tmp/models with space")
          assert.equal(options.env.SIMCTL_CHILD_PAPERCUT_EMBEDDING_TEST_CYCLES, "1")
          assert.equal(options.timeout, 660_000)
          assert.ok(args.includes("--exact"))
          if (failure === "inference") throw new Error("ETIMEDOUT")
          return completed
        }
        return ""
      }
      const check = () => checkIos({ executable, models: "/tmp/models with space" }, run)
      if (failure) assert.throws(check, /sign failed|ETIMEDOUT/)
      else check()
      assert.deepEqual(calls.at(-1).args, ["simctl", "delete", uuid])
      if (failure !== "codesign") assert.deepEqual(calls.at(-2).args, ["simctl", "shutdown", uuid])
      assert.ok(!existsSync(signedTest))
    }
  } finally {
    if (original === undefined) delete process.env[envKey]
    else process.env[envKey] = original
    rmSync(fixture, { recursive: true, force: true })
  }
})
