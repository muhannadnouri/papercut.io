import {
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

const state = vi.hoisted(() => ({ root: "" }))
vi.mock("../../lib/paths.js", async importOriginal => ({
  ...await importOriginal(),
  ROOT: state.root,
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
