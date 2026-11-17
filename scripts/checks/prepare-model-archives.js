import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import {
  chmod,
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  link,
  rm,
  stat,
  writeFile,
} from "node:fs/promises"
import {
  join,
  resolve,
} from "node:path"
import { parseArgs } from "node:util"

import { extractTar } from "../lib/archive.js"
import { downloadFile } from "../lib/download.js"
import { ROOT } from "../lib/paths.js"
import { isMain } from "../lib/process.js"

const INPUTS = join(ROOT, "src-tauri/tts/qualification-artifacts.json")

// Match the native phonemizer's path-and-content digest; reject links before copying any data.
export async function phonemizerDataDigest(root) {
  assert((await lstat(root)).isDirectory(), "Phonemizer data root must be a real directory")
  const files = []
  async function visit(directory, prefix) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = prefix + entry.name
      if (entry.isDirectory()) {
        await visit(join(directory, entry.name), path + "/")
      } else {
        assert(entry.isFile(), "Phonemizer data may not contain links: " + path)
        files.push(path)
      }
    }
  }
  await visit(root, "")
  const digest = createHash("sha256")
  for (const path of files.sort()) {
    digest.update(path + "\t" + sha256(await readFile(join(root, path))) + "\n")
  }
  return digest.digest("hex")
}

// SHA-256 also binds existing output bytes; never replace an archive under the same artifact name.
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex")
}

// A changed model needs a new artifact identity, not a --clobber path to an existing release.
export async function promoteArchive(temporary, destination) {
  try {
    await link(temporary, destination)
  } catch (error) {
    if (error.code !== "EEXIST") {
      throw error
    }
    assert.equal(sha256(await readFile(destination)), sha256(await readFile(temporary)), "Refusing to overwrite different archive bytes: " + destination)
  }
  await rm(temporary)
}

// Bind a prepared draft to committed catalog pins before CI can upload any assets.
export function assertPreparedArchives(prepared, manifest) {
  const { archives } = prepared
  assert.equal(archives.length, 2, "Both distill archives are required")
  assert.equal(new Set(archives.map((archive) => archive.modelName)).size, 2, "Duplicate prepared archive")
  for (const archive of archives) {
    assert.deepEqual(archive, manifest.archives.find((entry) => entry.modelName === archive.modelName), "Prepared archive differs from committed catalog: " + archive.modelName)
  }
  const names = new Set(archives.map((archive) => archive.modelName))
  assert.deepEqual(prepared.companions, (manifest.companions ?? []).filter((entry) => entry.modelNames.some((name) => names.has(name))), "Prepared source companion differs from committed catalog")
}

// Canonical permissions prevent a developer's umask or executable bits changing release hashes.
async function normalizePermissions(directory) {
  await chmod(directory, 0o755)
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      await normalizePermissions(path)
    } else {
      assert(entry.isFile(), "Archive staging may not contain links")
      await chmod(path, 0o644)
    }
  }
}

// Canonical tar metadata is shared by the model and corresponding-source archives.
async function assembleTar(work, outDir, name) {
  await normalizePermissions(join(work, name))
  const temporary = join(work, name + ".tar.bz2")
  execFileSync("tar", ["--sort=name", "--format=ustar", "--mtime=@0", "--owner=0", "--group=0", "--numeric-owner", "--mode=u=rwX,go=rX", "-cjf", temporary, "-C", work, name])
  const bytes = await readFile(temporary)
  await promoteArchive(temporary, join(outDir, name + ".tar.bz2"))
  return {
    sha256: sha256(bytes),
    archiveBytes: bytes.length,
  }
}

// Developer-only Linux/GNU tar assembly; the app downloads the resulting archives without these tools.
export async function prepareArchives({ sourceDir, dataDir, outDir }) {
  const inputs = JSON.parse(await readFile(INPUTS, "utf8"))
  const recipe = inputs.packaging
  assert.deepEqual(recipe.models.map((model) => model.language).sort(), ["ar", "en"], "English and Arabic must be prepared together")
  assert.match(recipe.releaseTag, /^[a-zA-Z0-9_.-]+$/)
  await mkdir(outDir, { recursive: true })
  const work = await mkdtemp(join(outDir, ".assembly-"))
  const archives = []
  try {
    const sources = []
    for (const pin of recipe.sourceArchives) {
      assert.match(pin.name, /^[a-zA-Z0-9_-]+$/)
      assert.equal(new URL(pin.url).protocol, "https:")
      const file = join(sourceDir, "shared", pin.name + (pin.url.endsWith(".zip") ? ".zip" : ".tar.gz"))
      await downloadFile({
        url: pin.url,
        dest: file,
        sha256: pin.sha256,
        label: pin.name,
        signal: AbortSignal.timeout(5 * 60 * 1000),
      })
      sources.push({
        ...pin,
        file,
      })
    }
    assert.equal(sources.length, 2, "Piper and eSpeak sources are both required")
    if (!dataDir) {
      const piper = sources.find((pin) => pin.name === "piper-phonemize")
      assert(piper.url.includes(recipe.dataSource.revision), "Data/source revision mismatch")
      const checkout = join(work, "piper-source")
      await mkdir(checkout)
      await extractTar({
        archive: piper.file,
        destination: checkout,
        compression: "gzip",
        stripComponents: 1,
      })
      dataDir = join(checkout, recipe.dataSource.path)
    }
    assert.equal(await phonemizerDataDigest(dataDir), recipe.dataTreeSha256, "Incorrect phonemizer data")
    for (const model of recipe.models) {
      assert.match(model.modelName, /^[a-zA-Z0-9_-]+$/)
      const directory = join(work, model.modelName)
      await mkdir(directory)
      const selected = inputs.assets.filter((asset) =>
        (asset.language === model.language && ["model.fp32.onnx", "tokens.txt", "voices.bin", "README.md"].includes(asset.file))
        || asset.language === "shared")
      assert.equal(selected.length, 6, "Incomplete model/license inputs")
      for (const asset of selected) {
        assert(["en", "ar", "shared"].includes(asset.language))
        assert.match(asset.file, /^[a-zA-Z0-9_.-]+$/)
        assert.equal(new URL(asset.url).protocol, "https:")
        const source = join(sourceDir, asset.language, asset.file)
        await downloadFile({
          url: asset.url,
          dest: source,
          sha256: asset.sha256,
          label: asset.file,
          signal: AbortSignal.timeout(5 * 60 * 1000),
        })
        assert.equal((await stat(source)).size, asset.bytes, "Incorrect pinned input size")
        await copyFile(source, join(directory, asset.file === "model.fp32.onnx" ? "model.onnx" : asset.file === "README.md" ? "MODEL_CARD.md" : asset.file))
      }
      assert.match(await readFile(join(directory, "MODEL_CARD.md"), "utf8"), /license: apache-2\.0/)
      await cp(dataDir, join(directory, "espeak-ng-data"), { recursive: true })
      const provenance = {
        schemaVersion: 1,
        modelId: "sherpa-onnx/" + model.modelName,
        language: model.language,
        precision: "fp32",
        speakerId: 0,
        voice: "af_msa",
        sourceFiles: selected,
        phonemizerData: {
          ...recipe.dataSource,
          treeSha256: recipe.dataTreeSha256,
        },
      }
      await writeFile(join(directory, "MODEL_SOURCE.json"), JSON.stringify(provenance, null, 2) + "\n")
      await writeFile(join(directory, "NOTICE.txt"),
        "Kokoro/Nabra distill model artifact for Papercut\n\n"
        + "Original model and voice: oddadmix; ONNX conversion: marwanelamami.\n"
        + "See MODEL_CARD.md and MODEL_SOURCE.json for the exact immutable source identities.\n"
        + "Weights, vocabulary and voice are unchanged; original model cards declare Apache-2.0.\n"
        + "eSpeak NG data is copied unchanged from the pinned published Piper source tree, under GPLv3 terms.\n"
        + "See LICENSE-APACHE-2.0 and COPYING-GPL-3.0; corresponding Piper/eSpeak sources accompany this release.\n"
        + "The application uses its native phoneme adapters and optional Libtashkeel; no Python is shipped here.\n")
      const packed = await assembleTar(work, outDir, model.modelName)
      archives.push({
        family: "kokoro",
        modelName: model.modelName,
        sourceLabel: "oddadmix / marwanelamami " + (model.language === "en" ? "Kokoro" : "Nabra") + " 7M FP32",
        url: "https://github.com/muhannadnouri/papercut.io/releases/download/" + recipe.releaseTag + "/" + model.modelName + ".tar.bz2",
        ...packed,
        platforms: ["desktop", "android"],
        licenseNote: "Apache-2.0 model/voice; bundled eSpeak NG data carries GPLv3 terms and corresponding source.",
      })
    }
    const sourceName = "piper-espeak-corresponding-source"
    const sourceRoot = join(work, sourceName)
    await mkdir(sourceRoot)
    for (const pin of sources) {
      await copyFile(pin.file, join(sourceRoot, pin.name + (pin.url.endsWith(".zip") ? ".zip" : ".tar.gz")))
    }
    await copyFile(join(sourceDir, "shared/COPYING-GPL-3.0"), join(sourceRoot, "COPYING-GPL-3.0"))
    await writeFile(join(sourceRoot, "MANIFEST.json"), JSON.stringify({
      scope: "Unmodified Piper data and corresponding Piper/eSpeak source for the model archives; no application binary is included.",
      dataSource: recipe.dataSource,
      dataTreeSha256: recipe.dataTreeSha256,
      sources: recipe.sourceArchives,
      modelArchives: archives.map(({ modelName, sha256 }) => ({
        modelName,
        sha256,
      })),
    }, null, 2) + "\n")
    const companion = await assembleTar(work, outDir, sourceName)
    const prepared = {
      schemaVersion: 2,
      archives,
      companions: [{
        family: "source",
        modelName: sourceName,
        url: "https://github.com/muhannadnouri/papercut.io/releases/download/" + recipe.releaseTag + "/" + sourceName + ".tar.bz2",
        ...companion,
        modelNames: archives.map((archive) => archive.modelName),
      }],
    }
    await writeFile(join(outDir, "model-manifest.json"), JSON.stringify(prepared, null, 2) + "\n")
    await writeFile(join(outDir, "SHA256SUMS.txt"), archives.map((a) => a.sha256 + "  " + a.modelName + ".tar.bz2").concat(companion.sha256 + "  " + sourceName + ".tar.bz2").join("\n") + "\n")
    return prepared
  } finally {
    await rm(work, {
      recursive: true,
      force: true,
    })
  }
}

// Source caches are disposable; only byte-verified immutable inputs enter the release artifacts.
async function main() {
  const { values } = parseArgs({
    options: {
      "source-dir": { type: "string" },
      "data-dir": { type: "string" },
      out: { type: "string" },
      "check-manifest": {
        type: "boolean",
        default: false,
      },
    },
  })
  assert(values.out, "Usage: --out <release directory> [--data-dir <verified Piper espeak-ng-data>] [--source-dir <cache>] [--check-manifest]")
  const prepared = await prepareArchives({
    sourceDir: resolve(values["source-dir"] ?? join(ROOT, ".cache/tts-model-sources")),
    dataDir: values["data-dir"] ? resolve(values["data-dir"]) : undefined,
    outDir: resolve(values.out),
  })
  if (values["check-manifest"]) {
    assertPreparedArchives(prepared, JSON.parse(await readFile(join(ROOT, "src-tauri/tts/model-manifest.json"), "utf8")))
  }
  for (const archive of prepared.archives) {
    console.log(archive.modelName + ": " + archive.archiveBytes + " bytes, sha256 " + archive.sha256)
  }
}

if (isMain(import.meta.url)) {
  await main()
}
