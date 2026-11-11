import { Context } from "effect"
export { Info, Ref, response } from "@opencode-ai/schema/location"

export class Service extends Context.Service<Service, unknown>()("@opencode/Location") {}
export * as Location from "./location"
