import {
  mkdtemp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import {
  dirname,
  join,
} from "node:path"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"

const state = vi.hoisted(() => ({
  root: "",
  download: vi.fn(),
  run: vi.fn(),
}))
vi.mock("../../download.js", () => ({ downloadFile: state.download }))
vi.mock("../../process.js", () => ({ run: state.run }))
vi.mock("../constants.js", async importOriginal => {
  const actual = await importOriginal()
  const mocked = { ...actual }
  for (const [key, value] of Object.entries(actual)) {
    if (typeof value === "string" && value.startsWith(actual.SHERPA_IOS_RUNTIME_ROOT)) {
      Object.defineProperty(mocked, key, { get: () => value.replace(actual.SHERPA_IOS_RUNTIME_ROOT, state.root) })
    }
  }
  return mocked
})

let constants
beforeEach(async () => {
  vi.resetModules()
  vi.clearAllMocks()
  // Test only the cache boundary; Linux deliberately stops before any Xcode/Piper source build.
  vi.stubGlobal("process", Object.create(process, { platform: { value: "linux" } }))
  state.root = await mkdtemp(join(tmpdir(), "papercut-ios-cache-"))
  constants = await import("../constants.js")
  state.run.mockImplementation(async (command, args) => {
    expect(command).toBe("unzip")
    const sherpa = args[1] === constants.SHERPA_IOS_ARCHIVE
    const framework = sherpa ? constants.SHERPA_IOS_SHERPA_XCFRAMEWORK : constants.SHERPA_IOS_ONNXRUNTIME_XCFRAMEWORK
    const name = sherpa ? "SherpaOnnxC.framework/SherpaOnnxC" : "onnxruntime.framework/onnxruntime"
    for (const slice of [constants.SHERPA_IOS_DEVICE_SLICE, constants.SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE]) {
      const file = join(framework, slice, name)
      await mkdir(dirname(file), { recursive: true })
      await writeFile(file, "!<arch>\nverified fixture")
    }
  })
})
afterEach(async () => {
  vi.unstubAllGlobals()
  await rm(state.root, {
    recursive: true,
    force: true,
  })
})

it("requires pinned extraction receipts and repairs missing, empty or changed cache files", async () => {
  const { ensureIosSherpaLibs } = await import("../sherpa.js")
  const prepareCache = () => expect(ensureIosSherpaLibs({ includeSimulator: false })).rejects.toThrow("requires macOS/Xcode")
  await prepareCache()
  expect(state.download).toHaveBeenCalledTimes(2)
  await prepareCache()
  expect(state.download).toHaveBeenCalledTimes(2)
  const receipt = join(state.root, ".archive-receipt.json")
  expect(JSON.parse(await readFile(receipt, "utf8")).files).toHaveLength(4)
  const file = join(constants.SHERPA_IOS_SHERPA_XCFRAMEWORK, constants.SHERPA_IOS_DEVICE_SLICE, "SherpaOnnxC.framework/SherpaOnnxC")
  for (const contents of ["", "changed library"]) {
    await writeFile(file, contents)
    await prepareCache()
  }
  expect(state.download).toHaveBeenCalledTimes(6)
  await rm(receipt)
  await prepareCache()
  expect(state.download).toHaveBeenCalledTimes(8)
})
