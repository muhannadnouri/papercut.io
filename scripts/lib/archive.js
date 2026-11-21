import { run } from "./process.js"

// Centralize tar flags so prep scripts only describe archive intent.
export async function extractTar({ archive, destination, compression, stripComponents }) {
  const args = [tarFlag(compression), archive, "-C", destination]
  if (stripComponents) {
    args.push("--strip-components=" + stripComponents)
  }
  await run("tar", args, { label: "tar " + args.join(" ") })
}

function tarFlag(compression) {
  if (compression === "gzip") {
    return "-xzf"
  }
  if (compression === "bzip2") {
    return "-xjf"
  }
  throw new Error("Unsupported tar compression: " + compression)
}
