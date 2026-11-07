import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import {
  cp,
  mkdir,
  readFile,
  rm,
} from "node:fs/promises"
import { execFileSync } from "node:child_process"
import { join } from "node:path"
import { ROOT } from "../lib/paths.js"
import {
  run,
  isMain,
} from "../lib/process.js"

// Gate-5 developer proof: two fresh builds of the source-only release path,
// wiped of all compilation products between runs, must produce byte-identical
// unsigned APKs. Sherpa compile determinism is additionally checked against
// the source recipe's recorded receipt hash. Cross-environment comparison is
// fdroidserver's rebuild against the published APK; this script covers one
// environment. Expect roughly 30-60 minutes: each run recompiles all Rust.

const CACHE = join(ROOT, ".cache/android-production")
const OUT_DIR = join(CACHE, "reproducibility")
const APK = join(
  ROOT,
  "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk",
)
// Only compilation products are wiped; sources, lockfiles and the committed
// Gradle scaffolding stay untouched so both runs see identical inputs.
const WIPED = [
  join(ROOT, "src-tauri/target/aarch64-linux-android"),
  join(ROOT, "src-tauri/gen/android/app/build"),
]
const SHERPA_LIB = "libsherpa-onnx-c-api.so"
const ORT_LIB = "libonnxruntime.so"

// "  27985944  Stored 27985944   0% 1981-01-01 01:01 f336ca24  lib/..." -> name -> {crc, size}
export function entryMap(unzipVerbose) {
  const map = new Map()
  for (const line of unzipVerbose.split("\n")) {
    const tokens = line.trim().split(/\s+/)
    if (tokens.length < 8 || !/^[0-9a-f]{8}$/i.test(tokens[6])) {
      continue
    }
    map.set(tokens.slice(7).join(" "), {
      crc: tokens[6].toLowerCase(),
      size: tokens[0],
    })
  }
  return map
}

export function compareEntries(a, b) {
  const problems = []
  for (const [name, info] of a) {
    const other = b.get(name)
    if (!other) {
      problems.push("only in first build: " + name)
    } else if (other.crc !== info.crc) {
      problems.push(`content differs: ${name} (crc ${info.crc} vs ${other.crc})`)
    } else if (other.size !== info.size) {
      problems.push("size differs: " + name)
    }
  }
  for (const name of b.keys()) {
    if (!a.has(name)) {
      problems.push("only in second build: " + name)
    }
  }
  return problems
}

async function sha256(file) {
  return createHash("sha256").update(await readFile(file)).digest("hex")
}

function unzipVerbose(apk) {
  return execFileSync("unzip", ["-v", apk], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  })
}

function packagedLibSha(apk, name) {
  return createHash("sha256")
    .update(execFileSync("unzip", ["-p", apk, "lib/arm64-v8a/" + name], {
      encoding: "buffer",
      maxBuffer: 512 * 1024 * 1024,
    }))
    .digest("hex")
}

async function buildOnce(label, env) {
  console.log(`[${label}] wiping ` + WIPED.join(", "))
  for (const dir of WIPED) {
    await rm(dir, {
      recursive: true,
      force: true,
    })
  }
  console.log(`[${label}] building (npm run android:apk:release, fresh Rust/Gradle)...`)
  await run("npm", ["run", "android:apk:release"], { env })
  const copy = join(OUT_DIR, `repro-${label}.apk`)
  await mkdir(OUT_DIR, { recursive: true })
  await cp(APK, copy)
  const summary = {
    apkSha256: await sha256(copy),
    sherpaLib: packagedLibSha(copy, SHERPA_LIB),
    ortLib: packagedLibSha(copy, ORT_LIB),
    entries: entryMap(unzipVerbose(copy)),
  }
  console.log(`[${label}] apk sha256 ` + summary.apkSha256)
  return summary
}

async function main() {
  const sourceLib = process.env.SHERPA_ONNX_LIB_DIR ?? join(CACHE, "source-arm64")
  assert(existsSync(join(sourceLib, SHERPA_LIB)), "Run scripts/fdroid/build-android-source-libs.js first")
  const receipt = JSON.parse(await readFile(join(CACHE, "source-arm64/source-receipt.json"), "utf8"))
  const expectedSherpa = receipt.artifacts[SHERPA_LIB].sha256
  assert(!process.env.SHERPA_ONNX_LIB_DIR || process.env.SHERPA_ONNX_LIB_DIR === join(CACHE, "source-arm64"),
    "This check compares against the source recipe receipt; use the default source-arm64 directory")

  await rm(OUT_DIR, {
    recursive: true,
    force: true,
  })
  const env = {
    ...process.env,
    SHERPA_ONNX_LIB_DIR: sourceLib,
    SHERPA_REQUIRE_SOURCE: "1",
  }
  const first = await buildOnce("1of2", env)
  const second = await buildOnce("2of2", env)

  assert.equal(first.sherpaLib, expectedSherpa,
    `Packaged ${SHERPA_LIB} does not match the recorded source-built hash`)
  assert.equal(first.sherpaLib, second.sherpaLib, `${SHERPA_LIB} differs between builds`)
  assert.equal(first.ortLib, second.ortLib, `${ORT_LIB} differs between builds`)

  const problems = compareEntries(first.entries, second.entries)
  if (problems.length) {
    console.error("APKs differ:")
    for (const problem of problems) {
      console.error("  - " + problem)
    }
  }
  assert.equal(first.apkSha256, second.apkSha256, "APK bytes differ between fresh builds (see entry report above)")
  console.log("PASS: two fresh builds produced byte-identical unsigned APKs " + first.apkSha256)
  console.log("Copies kept in .cache/android-production/reproducibility/ for signature-copying tests.")
}

if (isMain(import.meta.url)) {
  await main()
}
