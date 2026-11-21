import * as fs from "node:fs"
import * as fsPromises from "node:fs/promises"
import {
  cp,
  mkdtemp,
  mkdir,
  rm,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"

vi.mock("node:fs", { spy: true })
vi.mock("node:fs/promises", { spy: true })

const state = vi.hoisted(() => ({ root: "" }))
vi.mock("../../lib/paths.js", async importOriginal => ({
  ...await importOriginal(),
  get ROOT() {
    return state.root
  },
}))

let directory
beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), "papercut-license-notices-"))
  state.root = directory
})
afterEach(async () => {
  vi.resetModules()
  await rm(directory, {
    recursive: true,
    force: true,
  })
})

it("finds license files at the root before descending, bounded by depth", async () => {
  const { findLicenseFiles } = await import("../prepare-license-notices.js")
  await mkdir(join(directory, "sub", "deeper"), { recursive: true })
  await writeFile(join(directory, "LICENSE"), "root")
  await writeFile(join(directory, "sub", "deeper", "COPYING"), "deep")
  const found = findLicenseFiles(directory).map(path => path.split("/").pop())
  expect(found).toEqual(["LICENSE"])

  await rm(join(directory, "LICENSE"))
  // The depth bound counts recursion levels: one level short of the file misses it.
  expect(findLicenseFiles(directory, 1)).toHaveLength(0)
  expect(findLicenseFiles(directory, 2)).toHaveLength(1)
})

it("deduplicates identical texts and merges attribution in order", async () => {
  const { groupBySha } = await import("../prepare-license-notices.js")
  const groups = groupBySha([
    {
      text: "MIT text",
      attribution: "first",
    },
    {
      text: "MIT text",
      attribution: "second",
    },
    {
      text: "BSD text",
      attribution: "third",
    },
  ])
  expect(groups).toHaveLength(2)
  expect(groups[0].attribution).toEqual(["first", "second"])
  expect(groups[1].text).toBe("BSD text")
})

it("groups package attributions by declared license with sorted output", async () => {
  const { packageAttribution } = await import("../prepare-license-notices.js")
  const graph = new Map([
    ["pkg-b 2.0", "MIT OR Apache-2.0"],
    ["pkg-a 1.0", "Apache-2.0"],
    ["pkg-c 3.0", null],
  ])
  const lines = packageAttribution(new Map([...graph].filter(([, l]) => l)), "Rust")
  expect(lines).toEqual([
    "- Rust packages under Apache-2.0: pkg-a 1.0",
    "- Rust packages under MIT OR Apache-2.0: pkg-b 2.0",
  ])
})

it("requires reviewed license-file evidence for registry crates after upgrades", async () => {
  const { rustPackageLicense } = await import("../prepare-license-notices.js")
  const crate = {
    name: "language-tokenizer",
    version: "0.1.0",
    source: "registry+test",
  }
  expect(rustPackageLicense(crate)).toBe("WTFPL")
  expect(rustPackageLicense({
    ...crate,
    version: "0.1.1",
    license: "MIT",
  })).toBe("MIT")
  expect(() => rustPackageLicense({
    ...crate,
    version: "0.1.1",
  })).toThrow("Review missing Rust license evidence")
  expect(rustPackageLicense({
    name: "local-plugin",
    version: "0.1.0",
    source: null,
  })).toBeUndefined()
})

it("verifies committed ORT evidence without cache or network and rejects missing or changed files", async () => {
  const { verifyOrtLicenseEvidence } = await import("../prepare-license-notices.js")
  const evidence = join(directory, "src-tauri/licensing/onnxruntime")
  await cp(new URL("../../../src-tauri/licensing/onnxruntime/", import.meta.url), evidence, { recursive: true })
  const fetch = vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Network disabled"))
  try {
    await verifyOrtLicenseEvidence()
    expect(fetch).not.toHaveBeenCalled()
    await writeFile(join(evidence, "ThirdPartyNotices.txt"), "changed")
    await expect(verifyOrtLicenseEvidence()).rejects.toThrow("ONNX Runtime license evidence changed: ThirdPartyNotices.txt")
    await rm(join(evidence, "LICENSE"))
    await expect(verifyOrtLicenseEvidence()).rejects.toThrow("ENOENT")
  } finally {
    fetch.mockRestore()
  }
})

it("renders the verified ORT bytes even if the evidence files change or disappear afterward", async () => {
  const { verifyOrtLicenseEvidence, componentTexts } = await import("../prepare-license-notices.js")
  const evidence = join(directory, "src-tauri/licensing/onnxruntime")
  await cp(new URL("../../../src-tauri/licensing/onnxruntime/", import.meta.url), evidence, { recursive: true })
  const verified = await verifyOrtLicenseEvidence()
  await writeFile(join(evidence, "ThirdPartyNotices.txt"), "unverified replacement")
  await rm(join(evidence, "LICENSE"))
  const read = vi.spyOn(fsPromises, "readFile").mockImplementation(async path => {
    if (String(path).startsWith(evidence)) {
      throw new Error("ORT evidence must not be reread")
    }
    return "Other component license"
  })
  const exists = vi.spyOn(fs, "existsSync").mockReturnValue(true)
  const list = vi.spyOn(fs, "readdirSync").mockReturnValue([])
  try {
    const { blocks } = await componentTexts(verified)
    const sources = blocks.flatMap(block => block.sources)
    for (const [path, bytes] of verified) {
      const source = sources.find(source => source.attribution.startsWith(
        path.endsWith("ThirdPartyNotices.txt") ? "ONNX Runtime 8f027" : "ONNX Runtime source "))
      expect(source.text).toBe(bytes.toString("utf8").trimEnd() + "\n")
    }
  } finally {
    read.mockRestore()
    exists.mockRestore()
    list.mockRestore()
  }
})
