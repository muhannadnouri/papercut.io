import assert from "node:assert/strict"
import test from "node:test"
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { assertInferenceCompleted, checkAndroid, checkIos, command, simulatorDevice, testExecutable } from "./check-mobile-provider-inference.mjs"

const completed = [
  "lifecycle cycle complete: 1/1 Float32",
  "lifecycle cycle complete: 1/1 DynamicInt8",
  "index/query checks complete: Float32",
  "index/query checks complete: DynamicInt8",
  "test result: ok. 1 passed; 0 failed; 0 ignored;",
].join("\n")

test("Android compile step supplies NDK archive tools to native dependency builds", () => {
  const workflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8")
  const script = workflow.match(/name: Compile Android emulator provider regression[\s\S]*?run: \|\n([\s\S]*?)(?=\n      - name:)/)?.[1]
  assert.ok(script, "Find the actual Android compile step")
  const dir = mkdtempSync(join(tmpdir(), "papercut-android-toolchain-"))
  const ndk = join(dir, "NDK with spaces")
  try {
    // Capture the environment at the real Cargo boundary without needing a local NDK.
    const keys = ["CC_x86_64_linux_android", "CXX_x86_64_linux_android", "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER", "AR_x86_64_linux_android", "RANLIB_x86_64_linux_android"]
    const stub = `cargo() { "$TASK_NODE" -e 'console.log(JSON.stringify(Object.fromEntries(${JSON.stringify(keys)}.map(key => [key, process.env[key]]))))'; }\n`
    const env = { ...process.env, TASK_NODE: process.execPath, NDK_HOME: ndk, RUNNER_TEMP: dir }
    delete env.AR_x86_64_linux_android
    delete env.RANLIB_x86_64_linux_android
    const result = spawnSync("bash", ["-eu", "-c", stub + script], { env, encoding: "utf8", timeout: 5000 })
    assert.equal(result.status, 0, result.stderr)
    const configured = JSON.parse(readFileSync(join(dir, "android-provider-tests.json"), "utf8"))
    const bin = join(ndk, "toolchains/llvm/prebuilt/linux-x86_64/bin")
    assert.equal(configured.CC_x86_64_linux_android, join(bin, "x86_64-linux-android26-clang"))
    assert.equal(configured.CXX_x86_64_linux_android, join(bin, "x86_64-linux-android26-clang++"))
    assert.equal(configured.CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER, configured.CC_x86_64_linux_android)
    assert.equal(configured.AR_x86_64_linux_android, join(bin, "llvm-ar"))
    assert.equal(configured.RANLIB_x86_64_linux_android, join(bin, "llvm-ranlib"))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
})

test("native command wrapper preserves stderr markers and fails on exit errors and hangs", () => {
  assert.equal(command(process.execPath, ["-e", "process.stderr.write('marker')"], { combined: true }), "marker")
  assert.equal(command(process.execPath, ["-e", "process.stderr.write('diagnostic')"]), "")
  assert.throws(() => command(process.execPath, ["-e", "process.exit(3)"]), /exited with 3/)
  assert.throws(() => command(process.execPath, ["-e", `process.stdout.write(${JSON.stringify(completed)}); process.exit(134)`]), /exited with 134/)
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
  for (const invalid of ["", completed.replace("DynamicInt8", "Float32"), completed.replace("index/query checks complete: DynamicInt8", ""), completed.replace("1 passed", "0 passed"), completed.replace("0 failed", "1 failed")]) {
    assert.throws(() => assertInferenceCompleted(invalid))
  }
})

test("Android preserves native failures, captures crash logs and removes only owned emulator data", () => {
  for (const failure of [null, "push", "inference", "abort", "abort-diagnostics"]) {
    const calls = []
    const run = (program, args, options) => {
      assert.equal(program, "adb")
      assert.equal(args[0], "-s")
      assert.match(args[1], /^emulator-\d+$/)
      calls.push({ args, options })
      if (args.includes("getprop")) return "x86_64\n"
      if (failure === "push" && args.includes("push")) throw new Error("staging failed")
      if (args.includes("logcat") && failure === "abort-diagnostics") throw new Error("logcat failed")
      if (args[3]?.includes("timeout 600")) {
        if (failure === "inference") throw new Error("ETIMEDOUT")
        if (failure?.startsWith("abort")) throw new Error("adb exited with 134")
        return completed
      }
      return ""
    }
    const check = () => checkAndroid({ executable: "/tmp/test' file", models: "/tmp/models", runtimeLibs: "/tmp/libs" }, run)
    if (failure) assert.throws(check, /staging failed|ETIMEDOUT|adb exited with 134/)
    else check()
    const remote = calls.find(({ args }) => args.includes("mkdir")).args.at(-1)
    assert.match(remote, /^\/data\/local\/tmp\/papercut-provider-[0-9a-f-]{36}$/)
    assert.deepEqual(calls.at(-1).args.slice(2), ["shell", "rm", "-rf", remote])
    const diagnostics = calls.filter(({ args }) => args.includes("logcat"))
    assert.equal(diagnostics.length, failure ? 1 : 0)
    if (failure) {
      assert.deepEqual(calls.at(-2).args.slice(2), ["logcat", "-b", "crash", "-d", "-v", "threadtime"])
      assert.equal(diagnostics[0].options.timeout, 10_000)
    }
    if (!failure) {
      const inference = calls.find(({ options }) => options?.timeout === 660_000)
      assert.ok(inference.args[3].includes("PAPERCUT_EMBEDDING_TEST_DOWNLOAD=0"))
      assert.ok(inference.args[3].includes(`TMPDIR='${remote}'`))
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
          assert.equal(join(options.env.SIMCTL_CHILD_TMPDIR, "provider-test"), signedTest)
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

test("cleanup reports secondary errors, preserves native failures and still fails after successful inference", () => {
  const fixture = mkdtempSync(join(tmpdir(), "papercut-provider-cleanup-"))
  const executable = join(fixture, "test")
  writeFileSync(executable, "fixture")
  const originalLog = console.error
  const diagnostics = []
  console.error = message => diagnostics.push(message)
  try {
    for (const platform of ["android", "ios"]) {
      for (const inferenceFails of [false, true]) {
        diagnostics.length = 0
        const nativeError = new Error("native inference aborted")
        const cleanupError = new Error("cleanup disconnected")
        const calls = []
        let signedTest
        const run = (program, args) => {
          calls.push({ program, args })
          if (args.includes("getprop")) return "x86_64"
          if (args.includes("list")) return JSON.stringify({ devices: {
            "com.apple.CoreSimulator.SimRuntime.iOS-26-2": [{ isAvailable: true, deviceTypeIdentifier: "com.apple.CoreSimulator.SimDeviceType.iPhone-17" }],
          } })
          if (args.includes("create")) return "12345678-1234-1234-1234-123456789abc"
          if (program === "codesign") signedTest = args.at(-1)
          if (args.includes("spawn") || args.some(arg => arg.includes("timeout 600"))) {
            if (inferenceFails) throw nativeError
            return completed
          }
          if (args.includes("rm") || args.includes("shutdown")) throw cleanupError
          if (args.includes("delete")) throw new Error("delete failed too")
          return ""
        }
        const check = () => platform === "android"
          ? checkAndroid({ executable, models: "/tmp/models", runtimeLibs: "/tmp/libs" }, run)
          : checkIos({ executable, models: "/tmp/models" }, run)
        assert.throws(check, error => error === (inferenceFails ? nativeError : cleanupError))
        assert.ok(diagnostics.some(message => message.includes("cleanup disconnected")))
        if (platform === "ios") {
          assert.ok(calls.some(({ args }) => args.includes("delete")), "deletion still attempted after shutdown fails")
          assert.ok(diagnostics.some(message => message.includes("delete failed too")))
          assert.ok(!existsSync(signedTest), "host test copy removed despite both simulator cleanup failures")
        }
      }
    }
  } finally {
    console.error = originalLog
    rmSync(fixture, { recursive: true, force: true })
  }
})
