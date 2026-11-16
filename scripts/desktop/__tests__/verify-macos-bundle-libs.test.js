import {
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import {
  join,
  resolve,
} from "node:path"
import { spawnSync } from "node:child_process"
import { pathToFileURL } from "node:url"
import {
  expect,
  it,
} from "vitest"

const verifier = resolve("scripts/desktop/verify-macos-bundle-libs.js")

// Run the real verifier on a disposable bundle; host-independent command stubs model Mach-O/signing results.
function verifyBundle({ versioned = false, missingVersioned = false, missingOrt = false, missingPiper = false, signatures = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), "papercut-macos-verifier-"))
  try {
    const app = join(root, "Papercut.app")
    const resources = join(app, "Contents/Resources")
    const bin = join(root, "bin")
    mkdirSync(resources, { recursive: true })
    mkdirSync(join(app, "Contents/MacOS"))
    mkdirSync(bin)
    const names = ["libsherpa-onnx-c-api.dylib"]
    if (!missingOrt) {
      names.push("libonnxruntime.dylib")
    }
    if (!missingPiper) {
      names.push("libpiper_phonemize_core.dylib")
    }
    if (versioned && !missingVersioned) {
      names.push("libonnxruntime.1.27.0.dylib")
    }
    writeFileSync(join(app, "Contents/MacOS/app"), "fixture")
    for (const name of names) {
      writeFileSync(join(resources, name), "fixture")
    }
    const ort = versioned ? "libonnxruntime.1.27.0.dylib" : "libonnxruntime.dylib"
    const deps = {
      app: ["@rpath/libsherpa-onnx-c-api.dylib", "@rpath/libpiper_phonemize_core.dylib"],
      "libsherpa-onnx-c-api.dylib": ["@rpath/" + ort],
      "libonnxruntime.dylib": ["@rpath/" + ort, "/usr/lib/libSystem.B.dylib"],
    }
    const otool = `#!${process.execPath}
const path=require('node:path');const deps=${JSON.stringify(deps)};
const binary=process.argv[3];console.log(binary+':');
for(const dep of deps[path.basename(binary)]||[])console.log('\\t'+dep+' (compatibility version 1.0.0, current version 1.0.0)');
`
    writeFileSync(join(bin, "otool"), otool, { mode: 0o755 })
    writeFileSync(join(bin, "codesign"), `#!${process.execPath}\nconsole.error('fixture signature invalid');process.exit(1);\n`, { mode: 0o755 })
    const args = [verifier, app, ...(signatures ? ["--require-signatures"] : [])]
    const result = spawnSync(process.execPath, ["--input-type=module", "-e", `
      Object.defineProperty(process,'platform',{value:'darwin'});
      process.argv=[process.execPath,...${JSON.stringify(args)}];
      await import(${JSON.stringify(pathToFileURL(verifier).href)});
    `], {
      env: {
        ...process.env,
        PATH: bin + ":" + process.env.PATH,
      },
      encoding: "utf8",
      timeout: 5000,
    })
    expect(result.error).toBeUndefined()
    return {
      status: result.status,
      output: result.stdout + result.stderr,
    }
  } finally {
    rmSync(root, {
      recursive: true,
      force: true,
    })
  }
}

it("accepts Sherpa 2's unversioned ONNX Runtime dependency", () => {
  expect(verifyBundle().status).toBe(0)
})

it("accepts an older versioned dependency when its target is bundled", () => {
  expect(verifyBundle({ versioned: true }).status).toBe(0)
})

it("rejects a missing versioned target when Mach-O actually references it", () => {
  const result = verifyBundle({
    versioned: true,
    missingVersioned: true,
  })
  expect(result.status).toBe(1)
  expect(result.output).toContain("references missing dependency @rpath/libonnxruntime.1.27.0.dylib")
})

it("still requires the ONNX Runtime entry point", () => {
  const result = verifyBundle({ missingOrt: true })
  expect(result.status).toBe(1)
  expect(result.output).toContain("missing required dylib:")
})

it("still rejects a missing transitive Piper dependency", () => {
  const result = verifyBundle({ missingPiper: true })
  expect(result.status).toBe(1)
  expect(result.output).toContain("references missing dependency @rpath/libpiper_phonemize_core.dylib")
})

it("still verifies signatures when requested", () => {
  const result = verifyBundle({ signatures: true })
  expect(result.status).toBe(1)
  expect(result.output).toContain("codesign verification failed")
})
