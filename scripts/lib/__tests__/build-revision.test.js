import { execFileSync } from "node:child_process"
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"
import { productionBuildRevision } from "../build-revision.js"

let root
function git(...args) {
  return execFileSync("git", ["-c", "core.hooksPath=" + join(root, "no-hooks"),
    "-c", "commit.gpgsign=false", ...args], {
    cwd: root,
    encoding: "utf8",
  }).trim()
}
function init() {
  git("init", "--quiet")
  git("-c", "user.name=Revision Test", "-c", "user.email=revision@example.invalid",
    "commit", "--quiet", "--allow-empty", "-m", "fixture")
  return git("rev-parse", "HEAD")
}
beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "papercut-revision-test-"))
  vi.stubEnv("PAPERCUT_BUILD_REVISION", undefined)
})
afterEach(() => {
  vi.unstubAllEnvs()
  rmSync(root, {
    recursive: true,
    force: true,
  })
})

it("derives the full checkout revision independently of GitHub and inherited Git paths", () => {
  const revision = init()
  vi.stubEnv("GITHUB_SHA", "ab".repeat(20))
  vi.stubEnv("GIT_DIR", "/unrelated")
  vi.stubEnv("GIT_WORK_TREE", "/unrelated")
  vi.stubEnv("GIT_COMMON_DIR", "/unrelated")
  expect(productionBuildRevision(root)).toBe(revision)
})

it("accepts and canonicalizes an explicit revision only when it matches the checkout", () => {
  const revision = init()
  vi.stubEnv("PAPERCUT_BUILD_REVISION", revision.toUpperCase())
  expect(productionBuildRevision(root)).toBe(revision)
  vi.stubEnv("PAPERCUT_BUILD_REVISION", "ab".repeat(20))
  expect(() => productionBuildRevision(root)).toThrow("does not match")
})

it.each(["", "abcd1234", "x".repeat(40), " " + "ab".repeat(20)])("rejects malformed revision %j", value => {
  vi.stubEnv("PAPERCUT_BUILD_REVISION", value)
  expect(() => productionBuildRevision(root)).toThrow("full 40-character")
})

it("requires release metadata for an exported source tree, ignoring a parent's Git repository", () => {
  init()
  const exported = join(root, "source-export")
  mkdirSync(exported)
  expect(() => productionBuildRevision(exported)).toThrow("Source exports require")
  vi.stubEnv("PAPERCUT_BUILD_REVISION", "ab".repeat(20))
  expect(productionBuildRevision(exported)).toBe("ab".repeat(20))
})

it("resolves detached worktrees whose .git entry is a file", () => {
  const revision = init()
  const worktree = join(root, "worktree")
  git("worktree", "add", "--quiet", "--detach", worktree, revision)
  expect(productionBuildRevision(worktree)).toBe(revision)
})

it("fails on broken Git metadata instead of accepting an unchecked override", () => {
  writeFileSync(join(root, ".git"), "gitdir: missing\n")
  vi.stubEnv("PAPERCUT_BUILD_REVISION", "ab".repeat(20))
  expect(() => productionBuildRevision(root)).toThrow()
})
