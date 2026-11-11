import { plugin } from "bun"
import { resolve } from "node:path"
import { createHash } from "node:crypto"
import { readFileSync, readlinkSync } from "node:fs"

const source = "/upstream/opencode-v1.19.5"
const extra = "/fork-deps/source"
const pin = await Bun.file(resolve(import.meta.dir, "pin.json")).json()
const sourcePins = new Map(pin.sourceFiles.map((record: any) => [record.path, record.sha256]))
const extraPins = new Map(pin.supplementalSource.map((record: any) => [record.path, record.sha256]))
const loaded = new Set<string>()
const adapters = new Set<string>()
const sha256 = (contents: string) => createHash("sha256").update(contents).digest("hex")
const redirects: Record<string, string> = {
  "packages/core/src/location.ts": 'import { Context } from "effect"; export { Info, Ref, response } from "@opencode-ai/schema/location"; export class Service extends Context.Service<Service, unknown>()("@opencode/Location") {}; export * as Location from "./location"',
  "packages/core/src/session.ts": 'export * from "./session/schema"; export * as SessionV2 from "./session"',
  "packages/core/src/project.ts": 'export * from "./project/schema"; export * as ProjectV2 from "./project"; export * as Project from "./project"',
  "packages/opencode/src/snapshot/index.ts": 'export { Info as FileDiff } from "@opencode-ai/schema/file-diff"; export * as Snapshot from "./index"',
}
let canaryHidden = false
try { readFileSync(process.env.OPENCODE_FIXTURE_CANARY_PATH!) } catch (error) {
  if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  canaryHidden = true
}
const privateNetwork = readlinkSync("/proc/self/ns/net") !== process.env.OPENCODE_FIXTURE_PARENT_NETWORK
if (!canaryHidden || !privateNetwork || process.env.HOME !== "/fixture/home") throw new Error("Fixture isolation failed.")
await Bun.write("/fixture/isolation.json", JSON.stringify({ canaryHidden, privateNetwork, privateHome: true }) + "\n")

plugin({
  name: "explicit-fork-import-boundary",
  setup(builder) {
    builder.onLoad({ filter: /^\/(upstream\/opencode-v1\.19\.5|fork-deps\/source)\/.*\.(ts|js|mjs|cjs|json)$/ }, async (args) => {
      const contents = await Bun.file(args.path).text()
      const relative = args.path.startsWith(source + "/") ? args.path.slice(source.length + 1) : undefined
      if (relative) {
        if (sha256(contents) !== sourcePins.get(relative)) throw new Error(`Unpinned source import: ${relative}`)
        if (relative in redirects) {
          adapters.add(relative)
          return { contents: redirects[relative], loader: "ts" }
        }
        if (relative === "packages/opencode/src/storage/storage.ts") {
          const declaration = contents.slice(contents.indexOf("export class NotFoundError"), contents.indexOf("\nexport type Error"))
          if (sha256(declaration) !== pin.notFoundDeclarationSha256) throw new Error("The error declaration span changed.")
          adapters.add(relative)
          return { contents: 'import { Schema } from "effect"\n' + declaration, loader: "ts" }
        }
        loaded.add(relative)
      } else if (args.path.startsWith(extra + "/")) {
        const name = args.path.slice(extra.length + 1)
        if (sha256(contents) !== extraPins.get(name)) throw new Error(`Unpinned supplemental source: ${name}`)
        loaded.add(name)
      }
      return { contents, loader: args.path.endsWith(".ts") ? "ts" : args.path.endsWith(".json") ? "json" : "js" }
    })
  },
})
try {
  await import("./producer.ts")
} finally {
  await Bun.write("/fixture/loaded-source.json", JSON.stringify([...loaded].sort(), null, 2) + "\n")
  await Bun.write("/fixture/adapters.json", JSON.stringify([...adapters].sort(), null, 2) + "\n")
}
