import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { expect, it } from "vitest"

const workflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8")

// Execute the workflow's real Bash with an npm stub: no SDK, signing, models or network calls.
function script(name) {
  const start = workflow.indexOf("- name: " + name + "\n")
  expect(start).toBeGreaterThan(-1)
  const block = workflow.slice(start).match(/run: \|\n((?: {10}[^\n]*\n|\n)+)/)?.[1]
  expect(block).toBeTruthy()
  return block
}

it("mobile translation is an independent, default-off build input and never changes release builds", () => {
  expect(workflow).toMatch(/mobile_translation:\n\s+description: [^\n]+\n\s+type: boolean\n\s+default: false/)
  expect(workflow).toMatch(/provider_inference:\n\s+description: [^\n]+\n\s+type: boolean\n\s+default: false/)
  for (const job of ["build-android", "build-ios"]) {
    const body = workflow.slice(workflow.indexOf("  " + job + ":"))
    expect(body.match(/PAPERCUT_MOBILE_TRANSLATION: ([^\n]+)/)?.[1])
      .toBe("${{ github.event_name == 'workflow_dispatch' && inputs.mobile_translation }}")
  }
  const release = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8")
  expect(release).not.toMatch(/translation-worker|mobile_translation/)
})

it.runIf(process.platform !== "win32").each([
  ["Build Android APK", ["run", "android:apk:native-tts", "--"]],
  ["Build unsigned iOS simulator app with native TTS", ["run", "ios:ci", "--", "--native-tts"]],
  ["Build unsigned iOS device app with native TTS", ["run", "ios:ci:device", "--"]],
])("%s forwards the worker flag only for opt-in, independently of inference", (name, base) => {
  const stub = 'npm() { "$TASK_NODE" -e \'console.log(JSON.stringify(process.argv.slice(1)))\' "$@"; }\n'
  for (const enabled of ["false", "true"]) {
    for (const inference of ["false", "true"]) {
      const result = spawnSync("bash", ["-eu", "-c", stub + script(name)], {
        env: { ...process.env, TASK_NODE: process.execPath, PAPERCUT_MOBILE_TRANSLATION: enabled, PROVIDER_INFERENCE: inference },
        encoding: "utf8", timeout: 5000,
      })
      expect(result.error).toBeUndefined()
      expect(result.status, result.stderr).toBe(0)
      expect(JSON.parse(result.stdout)).toEqual([...base, ...(enabled === "true" ? ["--translation-worker"] : [])])
    }
  }
})

it.runIf(process.platform !== "win32").each([
  ["mobile", "true", true], ["full", "true", true],
  ["desktop", "true", false], ["cheap", "true", false], ["silma-runtime", "true", false],
  ["desktop", "false", true], ["cheap", "false", true],
])("dispatch scope=%s worker=%s validates before starting platform jobs", (scope, enabled, valid) => {
  const dir = mkdtempSync(join(tmpdir(), "papercut-worker-ci-"))
  try {
    const output = join(dir, "output")
    const result = spawnSync("bash", ["-eu", "-c", script("Detect changed CI surfaces")], {
      env: { ...process.env, EVENT_NAME: "workflow_dispatch", VALIDATION_SCOPE: scope, PAPERCUT_MOBILE_TRANSLATION: enabled, GITHUB_OUTPUT: output },
      encoding: "utf8", timeout: 5000,
    })
    expect(result.error).toBeUndefined()
    expect(result.status === 0, result.stderr).toBe(valid)
    if (!valid) expect(result.stderr).toContain("requires validation_scope mobile or full")
    else if (enabled === "true") {
      const flags = readFileSync(output, "utf8")
      expect(flags).toContain("android_packaging=true")
      expect(flags).toContain("ios_packaging=true")
    }
  } finally { rmSync(dir, { recursive: true, force: true }) }
})
