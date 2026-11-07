import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { lstatSync } from "node:fs"
import { join } from "node:path"
import { ROOT } from "./paths.js"

export function productionBuildRevision(root = ROOT) {
  const supplied = process.env.PAPERCUT_BUILD_REVISION
  assert(supplied === undefined || /^[a-f0-9]{40}$/i.test(supplied),
    "PAPERCUT_BUILD_REVISION must be a full 40-character Git revision")
  let checkout
  // An export inside another repository must not inherit that parent's revision.
  if (lstatSync(join(root, ".git"), { throwIfNoEntry: false })) {
    const env = { ...process.env }
    for (const name of ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"]) {
      delete env[name]
    }
    checkout = execFileSync("git", ["rev-parse", "--verify", "HEAD"], {
      cwd: root,
      env,
      encoding: "utf8",
    }).trim()
    assert(/^[a-f0-9]{40}$/.test(checkout), "Checkout has no full Git revision")
  }
  const revision = supplied?.toLowerCase() ?? checkout
  assert(revision, "Source exports require PAPERCUT_BUILD_REVISION from their release metadata")
  assert(!checkout || revision === checkout,
    "PAPERCUT_BUILD_REVISION does not match the checked-out source revision")
  return revision
}
