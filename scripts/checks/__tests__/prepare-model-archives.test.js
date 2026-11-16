import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import {
  mkdtemp,
  mkdir,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"

import { downloadFile } from "../../lib/download.js"

import {
  assertPreparedArchives,
  phonemizerDataDigest,
  promoteArchive,
} from "../prepare-model-archives.js"

const sha = (bytes) => createHash("sha256").update(bytes).digest("hex")

test("data identity covers sorted paths/content and rejects linked trees", async () => {
  const root = await mkdtemp(join(tmpdir(), "papercut-model-tree-"))
  try {
    await mkdir(join(root, "data"))
    await mkdir(join(root, "data/sub"))
    await writeFile(join(root, "data/z"), "last")
    await writeFile(join(root, "data/sub/a"), "first")
    const expected = sha("sub/a\t" + sha("first") + "\nz\t" + sha("last") + "\n")
    assert.equal(await phonemizerDataDigest(join(root, "data")), expected)
    await writeFile(join(root, "data/z"), "changed")
    assert.notEqual(await phonemizerDataDigest(join(root, "data")), expected)
    await symlink(join(root, "data"), join(root, "linked"))
    await assert.rejects(phonemizerDataDigest(join(root, "linked")), /real directory/)
    await symlink(join(root, "data/z"), join(root, "data/link"))
    await assert.rejects(phonemizerDataDigest(join(root, "data")), /may not contain links/)
  } finally {
    await rm(root, {
      recursive: true,
      force: true,
    })
  }
})

test("repeat assembly preserves an artifact and refuses changed bytes under its name", async () => {
  const root = await mkdtemp(join(tmpdir(), "papercut-model-promote-"))
  const output = join(root, "model.tar.bz2")
  const temporary = join(root, "temporary")
  try {
    await writeFile(temporary, "accepted")
    await promoteArchive(temporary, output)
    await writeFile(temporary, "accepted")
    await promoteArchive(temporary, output)
    await writeFile(temporary, "replacement")
    await assert.rejects(promoteArchive(temporary, output), /Refusing to overwrite/)
    assert.equal(await readFile(output, "utf8"), "accepted")
  } finally {
    await rm(root, {
      recursive: true,
      force: true,
    })
  }
})

test("draft uploads must match both committed models and their source companion", () => {
  const archives = [{
    modelName: "english",
    sha256: "a",
  }, {
    modelName: "arabic",
    sha256: "b",
  }]
  const companions = [{
    modelName: "source",
    sha256: "c",
    modelNames: ["english", "arabic"],
  }]
  const prepared = {
    archives,
    companions,
  }
  assertPreparedArchives(prepared, prepared)
  assert.throws(() => assertPreparedArchives({
    ...prepared,
    archives: [archives[0]],
  }, prepared), /Both distill/)
  assert.throws(() => assertPreparedArchives({
    ...prepared,
    archives: [archives[0], archives[0]],
  }, prepared), /Duplicate/)
  assert.throws(() => assertPreparedArchives({
    ...prepared,
    archives: [{
      ...archives[0],
      sha256: "changed",
    }, archives[1]],
  }, prepared), /differs from committed/)
  assert.throws(() => assertPreparedArchives({
    archives,
    companions: [],
  }, prepared), /source companion differs/)
})

test("aborted artifact transfers cannot leave a cache entry or partial file", async () => {
  const root = await mkdtemp(join(tmpdir(), "papercut-model-abort-"))
  const dest = join(root, "archive")
  const controller = new AbortController()
  controller.abort()
  try {
    await assert.rejects(downloadFile({
      url: "http://127.0.0.1:9/unused",
      dest,
      signal: controller.signal,
    }), { name: "AbortError" })
    await assert.rejects(readFile(dest), { code: "ENOENT" })
    await assert.rejects(readFile(dest + ".tmp-" + process.pid), { code: "ENOENT" })
  } finally {
    await rm(root, {
      recursive: true,
      force: true,
    })
  }
})
