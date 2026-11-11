import { plugin } from "bun"
import { resolve } from "node:path"
import { createHash } from "node:crypto"
import { readFileSync, readlinkSync } from "node:fs"

const source = "/upstream/opencode-v1.19.5"
const loaded = new Set<string>()
const pin = await Bun.file(resolve(import.meta.dir, "pin.json")).json()
const sourcePins = new Map<string, string>(
  pin.sourceFiles.map((record: { path: string; sha256: string }) => [record.path, record.sha256]),
)
let canaryHidden = false
try {
  readFileSync(process.env.OPENCODE_FIXTURE_CANARY_PATH!)
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  canaryHidden = true
}
const privateNetwork = readlinkSync("/proc/self/ns/net") !== process.env.OPENCODE_FIXTURE_PARENT_NETWORK
if (!canaryHidden || !privateNetwork || process.env.HOME !== "/fixture/home") {
  throw new Error("The private fixture isolation check failed.")
}
await Bun.write("/fixture/isolation.json", JSON.stringify({ canaryHidden, privateNetwork, privateHome: true }) + "\n")
plugin({
  name: "private-location-context",
  setup(builder) {
    builder.onLoad({ filter: /\.(ts|js|mjs|cjs|json)$/ }, async (args) => {
      if (args.path === `${source}/packages/core/src/location.ts`) {
        return { contents: await Bun.file(resolve(import.meta.dir, "location.ts")).text(), loader: "ts" }
      }
      const contents = await Bun.file(args.path).text()
      if (args.path.startsWith(`${source}/`)) {
        const relative = args.path.slice(source.length + 1)
        if (!pin.executedSourceFiles.includes(relative)) throw new Error(`Unpinned producer import: ${relative}`)
        if (createHash("sha256").update(contents).digest("hex") !== sourcePins.get(relative)) {
          throw new Error(`Producer import digest mismatch: ${relative}`)
        }
        loaded.add(relative)
      }
      const loader = args.path.endsWith(".ts") ? "ts" : args.path.endsWith(".json") ? "json" : "js"
      return { contents, loader }
    })
  },
})

try {
  await import("./producer.ts")
} finally {
  await Bun.write("/fixture/loaded-source.json", JSON.stringify([...loaded].sort(), null, 2) + "\n")
}
