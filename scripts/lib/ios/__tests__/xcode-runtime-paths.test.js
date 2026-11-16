import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { spawnSync } from "node:child_process"
import {
  expect,
  it,
} from "vitest"
import { iosSherpaLibDir } from "../sherpa.js"

const apple = resolve("src-tauri/gen/apple")
const project = readFileSync(resolve(apple, "project.yml"), "utf8")
const source = project.match(/      - script: \|\n([\s\S]*?)\n        name: Build Rust Code/)[1]
  .split("\n").map(line => line.replace(/^ {10}/, "")).join("\n")
const serialized = readFileSync(resolve(apple, "app.xcodeproj/project.pbxproj"), "utf8")
const phase = [...serialized.matchAll(/shellScript = ("(?:\\.|[^"\\])*");/g)]
  .map(match => JSON.parse(match[1])).find(script => script.includes("SHERPA_ONNX_IOS_SLICE"))

// Execute the committed shell phase, stubbing only the final Tauri call; path selection stays real.
function runPhase(platform, skip = false, sourceRoot = apple) {
  return spawnSync("/bin/sh", ["-c", 'npm() { printf "%s\\n" "$SHERPA_ONNX_LIB_DIR" "$ORT_LIB_LOCATION"; };\n' + phase], {
    env: {
      ...process.env,
      SRCROOT: sourceRoot,
      PLATFORM_NAME: platform,
      PLATFORM_DISPLAY_NAME: "fixture",
      SDKROOT: "/fixture-sdk",
      FRAMEWORK_SEARCH_PATHS: "/fixture-frameworks",
      HEADER_SEARCH_PATHS: "/fixture-headers",
      CONFIGURATION: "Release",
      ARCHS: "arm64",
      SHERPA_ONNX_LIB_DIR: "/stale-wrong-slice",
      ORT_LIB_LOCATION: "/stale-ort",
      PAPERCUT_SKIP_TAURI_IOS_XCODE_SCRIPT: skip ? "1" : "0",
    },
    encoding: "utf8",
    timeout: 5000,
  })
}

it("keeps the XcodeGen source and committed Xcode build phase synchronized", () => {
  expect(phase).toBe(source)
})

it.each([
  ["iphonesimulator", "ios-arm64-simulator"],
  ["iphoneos", "ios-arm64"],
])("uses the prepared runtime for %s rather than an inherited or old path", (platform, slice) => {
  const result = runPhase(platform)
  expect(result.error).toBeUndefined()
  expect(result.status, result.stderr).toBe(0)
  const expected = iosSherpaLibDir(slice)
  expect(result.stdout.trim().split("\n")).toEqual([expected, expected])
})

it("preserves the skip path for an already prepared libapp.a", () => {
  const result = runPhase("iphonesimulator", true)
  expect(result.error).toBeUndefined()
  expect(result.status).toBe(0)
  expect(result.stdout).toContain("Skipping Tauri iOS xcode-script")
  expect(result.stdout).not.toContain("stale-ort")
})

it("does not invoke Tauri when the project root cannot be resolved", () => {
  const result = runPhase("iphonesimulator", false, "")
  expect(result.error).toBeUndefined()
  expect(result.status).not.toBe(0)
  expect(result.stdout).toBe("")
})
