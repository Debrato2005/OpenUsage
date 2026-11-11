import { Effect, Layer } from "effect"
import { mkdirSync, copyFileSync } from "node:fs"
import { Database } from "/upstream/opencode-v1.19.5/packages/core/src/database/database.ts"
import { EventV2 } from "/upstream/opencode-v1.19.5/packages/core/src/event.ts"
import { SessionProjector } from "/upstream/opencode-v1.19.5/packages/core/src/session/projector.ts"
import { ProjectTable } from "/upstream/opencode-v1.19.5/packages/core/src/project/sql.ts"
import { EventTable } from "/upstream/opencode-v1.19.5/packages/core/src/event/sql.ts"
import { SessionTable, MessageTable, PartTable, SessionMessageTable } from "/upstream/opencode-v1.19.5/packages/core/src/session/sql.ts"
import { Session } from "/upstream/opencode-v1.19.5/packages/opencode/src/session/session.ts"
import { BackgroundJob } from "/upstream/opencode-v1.19.5/packages/opencode/src/background/job.ts"
import { RuntimeFlags } from "/upstream/opencode-v1.19.5/packages/opencode/src/effect/runtime-flags.ts"
import { EventV2Bridge } from "/upstream/opencode-v1.19.5/packages/opencode/src/event-v2-bridge.ts"
import { InstanceRef } from "/upstream/opencode-v1.19.5/packages/opencode/src/effect/instance-ref.ts"
import { MessageID, PartID } from "/upstream/opencode-v1.19.5/packages/opencode/src/session/schema.ts"

const ctx = { directory: "/fixture/project", worktree: "/fixture/project", project: { id: "synthetic-forks" } }
const tokens = { input: 120, output: 30, reasoning: 11, cache: { read: 7, write: 5 } }
const localTokens = { input: 10, output: 3, reasoning: 1, cache: { read: 2, write: 1 } }
const model = { providerID: "anthropic", modelID: "claude-sonnet-4-20250514" }
const time = 1794355200000
const output: Record<string, unknown> = {}
let exported: EventV2.SerializedEvent[] = []

for (const scenario of ["full_fork", "bounded_fork", "before_usage", "local_usage", "distinct_calls", "replayed"]) {
  const directory = `/fixture/${scenario}`
  mkdirSync(directory, { recursive: true })
  const database = Database.layerFromPath(`${directory}/opencode.db`)
  const events = EventV2.layerWith().pipe(Layer.provideMerge(database))
  const projected = SessionProjector.node.implementation!.pipe(Layer.provideMerge(events))
  const bridge = EventV2Bridge.node.implementation!.pipe(Layer.provideMerge(projected))
  const dependencies = Layer.mergeAll(bridge, BackgroundJob.node.implementation!, RuntimeFlags.layer())
  const session = Session.node.implementation!.pipe(Layer.provideMerge(dependencies))
  output[scenario] = await Effect.runPromise(Effect.gen(function* () {
    const { db } = yield* Database.Service
    const api = yield* Session.Service
    const events = yield* EventV2.Service
    yield* db.insert(ProjectTable).values({ id: ctx.project.id, worktree: ctx.worktree, sandboxes: [] }).run()
    let operation: Record<string, unknown> = { replayCount: 2 }
    if (scenario === "replayed") {
      for (const aggregate of new Set(exported.map((event) => event.aggregateID))) {
        const rows = exported.filter((event) => event.aggregateID === aggregate)
        yield* events.replayAll(rows)
        yield* events.replayAll(rows)
      }
    } else {
      const parent = yield* api.create({ title: "Synthetic parent" })
      const userID = MessageID.ascending("msg_synthetic_user_1")
      const assistantID = MessageID.ascending("msg_synthetic_assistant_1")
      const boundaryID = MessageID.ascending("msg_synthetic_user_2")
      const user = (id: any, created: number) => ({ id, sessionID: parent.id, role: "user" as const,
        time: { created }, agent: "build", model })
      yield* api.updateMessage(user(userID, time))
      yield* api.updatePart({ id: PartID.ascending("prt_synthetic_user_1"), messageID: userID,
        sessionID: parent.id, type: "text", text: "Synthetic prompt" })
      const assistant = (id: any, sessionID: any, parentID: any, value: typeof tokens, created: number) => ({
        id, sessionID, parentID, role: "assistant" as const, time: { created, completed: created + 1 },
        ...model, agent: "build", mode: "build", path: { cwd: ctx.directory, root: ctx.worktree },
        finish: "stop", cost: 0, tokens: value,
      })
      yield* api.updateMessage(assistant(assistantID, parent.id, userID, tokens, time + 1000))
      yield* api.updatePart({ id: PartID.ascending("prt_synthetic_finish_1"), messageID: assistantID,
        sessionID: parent.id, type: "step-finish", reason: "stop", cost: 0, tokens })
      yield* api.updateMessage(user(boundaryID, time + 2000))
      yield* api.updatePart({ id: PartID.ascending("prt_synthetic_user_2"), messageID: boundaryID,
        sessionID: parent.id, type: "text", text: "Synthetic follow-up" })
      if (scenario === "full_fork") {
        yield* db.run("PRAGMA wal_checkpoint(TRUNCATE)")
        mkdirSync("/fixture/parent_only", { recursive: true })
        copyFileSync(`${directory}/opencode.db`, "/fixture/parent_only/opencode.db")
      }
      if (scenario === "distinct_calls") {
        const independent = yield* api.create({ title: "Synthetic independent session" })
        const nextUser = MessageID.ascending()
        const id = MessageID.ascending()
        yield* api.updateMessage({ ...user(nextUser, time), sessionID: independent.id })
        yield* api.updatePart({ id: PartID.ascending(), messageID: nextUser, sessionID: independent.id,
          type: "text", text: "Synthetic prompt" })
        yield* api.updateMessage(assistant(id, independent.id, nextUser, tokens, time + 1000))
        yield* api.updatePart({ id: PartID.ascending(), messageID: id, sessionID: independent.id,
          type: "step-finish", reason: "stop", cost: 0, tokens })
        operation = { parentID: parent.id, independentID: independent.id, forkInvoked: false, newUsage: 173 }
      } else {
        const boundary = scenario === "bounded_fork" ? boundaryID : scenario === "before_usage" ? assistantID : undefined
        const fork = yield* api.fork({ sessionID: parent.id, ...(boundary ? { messageID: boundary } : {}) })
        const forkMessages = yield* api.messages({ sessionID: fork.id })
        operation = { parentID: parent.id, forkID: fork.id, boundaryID: boundary ?? null,
          inheritedMessageIDs: forkMessages.map((message) => message.info.id), newUsage: scenario === "local_usage" ? 17 : 0 }
        if (scenario === "local_usage") {
          const id = MessageID.ascending()
          yield* api.updateMessage(assistant(id, fork.id, forkMessages.at(-1)!.info.id, localTokens, time + 3000))
          yield* api.updatePart({ id: PartID.ascending(), messageID: id, sessionID: fork.id,
            type: "step-finish", reason: "stop", cost: 0, tokens: localTokens })
        }
      }
    }
    const sessions = yield* db.select().from(SessionTable).all()
    const messages = yield* db.select().from(MessageTable).all()
    const parts = yield* db.select().from(PartTable).all()
    const v2messages = yield* db.select().from(SessionMessageTable).all()
    const durable = yield* db.select().from(EventTable).all()
    if (scenario === "full_fork") exported = durable.map((row) => ({
      id: row.id, type: row.type, seq: row.seq, aggregateID: row.aggregate_id, data: row.data,
    }))
    yield* db.run("PRAGMA wal_checkpoint(TRUNCATE)")
    return {
      operation,
      sessions: sessions.map((row) => ({ id: row.id, parentID: row.parent_id, metadata: row.metadata,
        keys: Object.keys(row).sort(), tokens: [row.tokens_input, row.tokens_output, row.tokens_reasoning,
          row.tokens_cache_read, row.tokens_cache_write] })),
      messages: messages.map((row) => ({ id: row.id, sessionID: row.session_id, role: row.data.role,
        parentID: "parentID" in row.data ? row.data.parentID : null, keys: Object.keys(row.data).sort(),
        tokens: "tokens" in row.data ? row.data.tokens : null })),
      parts: parts.map((row) => ({ id: row.id, sessionID: row.session_id, messageID: row.message_id,
        type: row.data.type, keys: Object.keys(row.data).sort(), tokens: "tokens" in row.data ? row.data.tokens : null })),
      events: durable.map((row) => ({ type: row.type, keys: Object.keys(row.data).sort() })),
      eventCount: durable.length, v2MessageCount: v2messages.length,
    }
  }).pipe(Effect.provideService(InstanceRef, ctx), Effect.provide(session), Effect.scoped))
}
await Bun.write("/fixture/producer-result.json", JSON.stringify(output, null, 2) + "\n")
