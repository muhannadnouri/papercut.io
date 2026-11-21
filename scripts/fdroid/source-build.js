import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import {
  mkdir,
  readFile,
  writeFile,
} from "node:fs/promises"
import { join } from "node:path"
import { ROOT } from "../lib/paths.js"
import { run } from "../lib/process.js"

export const SOURCE_CACHE = join(ROOT, ".cache/android-production")

// A source override must not quietly reuse another revision or local tracked edits.
export function assertSourceState(actual, expected, dirty, untracked = "") {
  assert.equal(actual.trim(), expected, "Source checkout differs from the pinned revision")
  assert.equal(dirty.trim(), "", "Source checkout has tracked edits; use a clean checkout")
  assert.equal(untracked.trim(), "", "Source checkout has untracked files; use a clean checkout")
}

// Fetch immutable Git objects; existing checkouts are inspected, never reset or overwritten.
export async function source(name, url, revision, submodules = false) {
  assert(/^[a-f0-9]{40}$/.test(revision), "Source revisions must be full commit IDs")
  const directory = join(SOURCE_CACHE, name)
  if (!existsSync(directory)) {
    await mkdir(directory, { recursive: true })
    await run("git", ["init", directory])
    await run("git", ["-C", directory, "remote", "add", "origin", url])
    await run("git", ["-C", directory, "fetch", "--depth=1", "origin", revision])
    await run("git", ["-C", directory, "checkout", "--detach", "FETCH_HEAD"])
  }
  // PDF.js tracks one CRLF .link despite its LF attributes; ignore only end-of-line CR normalization.
  const diff = ["diff", "--no-ext-diff", "--ignore-cr-at-eol", "HEAD", "--", "."]
  assertSourceState(
    output("git", ["rev-parse", "HEAD"], directory), revision,
    output("git", diff, directory),
    output("git", ["ls-files", "--others", "--exclude-standard"], directory),
  )
  if (submodules) {
    await run("git", ["submodule", "update", "--init", "--recursive", "--depth=1"], { cwd: directory })
    assert(!/^[+U-]/m.test(output("git", ["submodule", "status", "--recursive"], directory)), "Submodule revision differs from its parent source pin")
    assert.equal(output("git", diff, directory).trim(), "", "Source/submodules have tracked edits")
  }
  return directory
}

// Small metadata queries stay separate from inherited build output and never invoke a shell.
export function output(file, args, cwd = ROOT, env = process.env) {
  return execFileSync(file, args, {
    cwd,
    env,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  })
}

// Both OCR and PDF recipes use the same compiler version, independent of Docker's floating tags.
export function emscriptenEnv() {
  assert(process.env.EMSDK, "Set EMSDK to an activated Emscripten 4.0.15 installation")
  const sdk = process.env.EMSDK
  const env = {
    ...process.env,
    EMSDK: sdk,
    EM_CONFIG: join(sdk, ".emscripten"),
    PATH: [join(sdk, "upstream/emscripten"), join(sdk, "upstream/bin"), process.env.PATH].join(":"),
    CMAKE_POLICY_VERSION_MINIMUM: "3.5",
    CMAKE_BUILD_PARALLEL_LEVEL: "2",
    MAKEFLAGS: "-j2",
  }
  assert(/emcc .*\b4\.0\.15\b/.test(output("emcc", ["--version"], ROOT, env)), "Expected Emscripten 4.0.15")
  return env
}

// Output hashes describe this build; they are not reference hashes or clean-build reproducibility proof.
export async function receipt(directory, sources, files, tools) {
  const artifacts = {}
  for (const file of files) {
    const bytes = await readFile(join(directory, file))
    assert(bytes.length > 0, "Empty source-built output: " + file)
    artifacts[file] = {
      bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    }
  }
  await writeFile(join(directory, "source-receipt.json"), JSON.stringify({
    sources,
    tools: {
      node: process.version,
      ...tools,
    },
    artifacts,
  }, null, 2) + "\n")
}
