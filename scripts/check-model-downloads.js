import assert from "node:assert/strict"
import { readFile, stat } from "node:fs/promises"
import { resolve, join } from "node:path"
import { parseArgs } from "node:util"
import { pathToFileURL } from "node:url"
import { downloadFile } from "./lib/download.js"

export function validatedArchives(manifest, family) {
  assert.equal(manifest.schemaVersion, 2, "Unsupported model manifest")
  const names = new Set()
  for (const { modelName, url, fallbackUrl, sha256, archiveBytes } of manifest.archives) {
    assert.match(modelName, /^[a-zA-Z0-9_-]+$/)
    assert.match(sha256, /^[a-f0-9]{64}$/)
    assert(Number.isSafeInteger(archiveBytes) && archiveBytes > 0)
    for (const source of [url, fallbackUrl].filter(Boolean)) assert.equal(new URL(source).protocol, "https:")
    assert(!names.has(modelName), `Duplicate archive: ${modelName}`)
    names.add(modelName)
  }
  const archives = manifest.archives.filter((archive) => !family || archive.family === family)
  assert(archives.length > 0, "No matching model archives")
  return archives
}

export function assertReleaseAsset(archive, release, source) {
  const name = decodeURIComponent(new URL(source).pathname.split("/").at(-1))
  const asset = release.assets.find((asset) => asset.name === name)
  assert(asset, `Missing release asset: ${source}`)
  assert.equal(asset.state, "uploaded", `${name}: upload incomplete`)
  assert.equal(asset.size, archive.archiveBytes, `${name}: size changed`)
  assert.equal(asset.digest, `sha256:${archive.sha256}`, `${name}: checksum changed`)
  if (new URL(source).pathname.startsWith("/muhannadnouri/papercut.io/")) {
    assert.equal(release.immutable, true, "Papercut model release must be immutable before shipping")
  }
}

async function main() {
  const { values } = parseArgs({ options: {
    "download-dir": { type: "string" },
    family: { type: "string" },
    upstream: { type: "boolean", default: false },
  } })
  const manifest = JSON.parse(await readFile(new URL("../src-tauri/tts/model-manifest.json", import.meta.url), "utf8"))
  const archives = validatedArchives(manifest, values.family)
  const releases = new Map()
  for (const archive of archives) {
    const source = values.upstream ? archive.fallbackUrl ?? archive.url : archive.url
    const url = new URL(source)
    const match = url.pathname.match(/^\/([^/]+)\/([^/]+)\/releases\/download\/([^/]+)\/[^/]+$/)
    assert(url.hostname === "github.com" && match, `Expected a GitHub release URL: ${source}`)
    const [, owner, repo, tag] = match
    const api = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${tag}`
    if (!releases.has(api)) {
      const response = await fetch(api, {
        headers: { Accept: "application/vnd.github+json", ...(process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {}) },
        signal: AbortSignal.timeout(30_000),
      })
      assert(response.ok, `${api}: HTTP ${response.status}`)
      releases.set(api, await response.json())
    }
    assertReleaseAsset(archive, releases.get(api), source)
    console.log(`PASS metadata: ${archive.modelName}`)
    if (values["download-dir"]) {
      const dest = join(resolve(values["download-dir"]), `${archive.modelName}.tar.bz2`)
      // Always exercise a fresh transfer. A warm cache would hide broken URLs.
      await downloadFile({ url: source, dest, force: true, sha256: archive.sha256, label: archive.modelName })
      assert.equal((await stat(dest)).size, archive.archiveBytes, `${archive.modelName}: downloaded size changed`)
      console.log(`PASS download: ${archive.modelName}`)
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => { console.error(error.message); process.exitCode = 1 })
}
