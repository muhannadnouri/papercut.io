import assert from "node:assert/strict"
import test from "node:test"
import { validatedArchives, assertReleaseAsset } from "./check-model-downloads.js"

test("archives are unique and changed/missing/mutable releases fail monitoring", () => {
  const model = {
    family: "kokoro", modelName: "kokoro-v1_0",
    url: "https://github.com/muhannadnouri/papercut.io/releases/download/model-v1/model.tar.bz2",
    sha256: "a".repeat(64), archiveBytes: 42,
  }
  const manifest = { schemaVersion: 2, archives: [model, { ...model, family: "vits", modelName: "piper" }] }
  assert.equal(validatedArchives(manifest).length, 2)
  const [archive] = validatedArchives(manifest, "kokoro")
  assert.deepEqual(validatedArchives(manifest, "kokoro"), [model])
  assert.throws(() => validatedArchives({ ...manifest, archives: [model, { ...model }] }), /Duplicate archive/)
  assert.throws(() => validatedArchives({ ...manifest, schemaVersion: 1 }), /Unsupported model manifest/)
  assert.throws(() => validatedArchives(manifest, "unknown"), /No matching model archives/)
  const asset = { name: "model.tar.bz2", state: "uploaded", size: 42, digest: `sha256:${model.sha256}` }
  const release = { immutable: true, assets: [asset] }
  assertReleaseAsset(archive, release, model.url)
  for (const broken of [
    { ...release, immutable: false },
    { ...release, assets: [] },
    { ...release, assets: [{ ...asset, size: 43 }] },
    { ...release, assets: [{ ...asset, digest: `sha256:${"b".repeat(64)}` }] },
  ]) assert.throws(() => assertReleaseAsset(archive, broken, model.url))
})
