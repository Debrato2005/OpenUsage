import { Effect, Layer, Schema } from "effect"
import { mkdirSync } from "node:fs"
import { Database } from "/upstream/opencode-v1.19.5/packages/core/src/database/database.ts"
import { EventV2 } from "/upstream/opencode-v1.19.5/packages/core/src/event.ts"
import { SessionProjector } from "/upstream/opencode-v1.19.5/packages/core/src/session/projector.ts"
import { SessionEvent } from "/upstream/opencode-v1.19.5/packages/core/src/session/event.ts"
import { SessionV1 } from "/upstream/opencode-v1.19.5/packages/core/src/v1/session.ts"
import { ProjectTable } from "/upstream/opencode-v1.19.5/packages/core/src/project/sql.ts"
import { EventTable } from "/upstream/opencode-v1.19.5/packages/core/src/event/sql.ts"
import { SessionInputTable, SessionMessageTable } from "/upstream/opencode-v1.19.5/packages/core/src/session/sql.ts"

const time = 1794355200000
const sessionID = "ses_synthetic_runtime"
const model = { id: "claude-sonnet-4-20250514", providerID: "anthropic" }
const location = { directory: "/fixture/project" }
const tokens = { input: 120, output: 30, reasoning: 11, cache: { read: 7, write: 5 } }
const scenarios = [
  "completed", "user_peer", "repeated_save", "changed_save", "distinct_calls", "reported_zero",
  "unfinished", "failed", "agent_switched", "model_switched", "system", "synthetic", "shell", "compaction",
  "replayed",
]
const result: Record<string, unknown> = {}
let exported: EventV2.SerializedEvent[] = []

for (const scenario of scenarios) {
  const directory = `/fixture/${scenario}`
  mkdirSync(directory, { recursive: true })
  const database = Database.layerFromPath(`${directory}/opencode.db`)
  const eventsLayer = EventV2.layerWith().pipe(Layer.provideMerge(database))
  const projected = SessionProjector.node.implementation!.pipe(Layer.provideMerge(eventsLayer))
  result[scenario] = await Effect.runPromise(
    Effect.gen(function* () {
      const { db } = yield* Database.Service
      const events = yield* EventV2.Service
      let sequence = 0
      const publish = (definition: any, fields: Record<string, unknown>) => events.publish(
        definition,
        Schema.decodeUnknownSync(definition.data)({ sessionID, timestamp: time + sequence * 1000, ...fields }),
        { location, id: EventV2.ID.make(`evt_synthetic_${sequence++}`) },
      )
      const start = (id = "msg_synthetic_assistant") => publish(SessionEvent.Step.Started, {
        assistantMessageID: id, agent: "build", model,
      })
      const end = (value = tokens, id = "msg_synthetic_assistant") => publish(SessionEvent.Step.Ended, {
        assistantMessageID: id, finish: "stop", cost: 0, tokens: value,
      })
      yield* db.insert(ProjectTable).values({ id: "synthetic-runtime", worktree: location.directory, sandboxes: [] }).run()
      if (scenario === "replayed") {
        yield* events.replayAll(exported)
        yield* events.replayAll(exported)
      } else {
        const info = SessionV1.SessionInfo.make({
          id: sessionID, slug: "synthetic-runtime", version: "1.19.5", projectID: "synthetic-runtime",
          directory: location.directory, title: "Synthetic runtime", time: { created: time, updated: time },
        })
        yield* publish(SessionV1.Event.Created, { info })
        if (scenario === "user_peer") {
          yield* publish(SessionEvent.Prompted, { messageID: "msg_synthetic_user", prompt: { text: "Synthetic prompt" }, delivery: "queue" })
        }
        yield* start()
        if (scenario === "failed") {
          yield* publish(SessionEvent.Step.Failed, { assistantMessageID: "msg_synthetic_assistant", error: { type: "unknown", message: "Synthetic failure" } })
        } else if (scenario !== "unfinished") {
          yield* end(scenario === "reported_zero" ? { input: 0, output: 0, reasoning: 0, cache: { read: 0, write: 0 } } : tokens)
        }
        if (scenario === "repeated_save" || scenario === "changed_save") {
          yield* end()
          yield* end(scenario === "changed_save" ? { ...tokens, input: 137 } : tokens)
        }
        if (scenario === "distinct_calls") {
          yield* start("msg_synthetic_second")
          yield* end(tokens, "msg_synthetic_second")
        }
        if (scenario === "agent_switched") {
          yield* publish(SessionEvent.AgentSwitched, { messageID: "msg_synthetic_agent", agent: "plan" })
        }
        if (scenario === "model_switched") {
          yield* publish(SessionEvent.ModelSwitched, { messageID: "msg_synthetic_model", model })
        }
        if (scenario === "system") {
          yield* publish(SessionEvent.ContextUpdated, { messageID: "msg_synthetic_system", text: "Synthetic system context" })
        }
        if (scenario === "synthetic") {
          yield* publish(SessionEvent.Synthetic, { messageID: "msg_synthetic_note", text: "Synthetic note" })
        }
        if (scenario === "shell") {
          yield* publish(SessionEvent.Shell.Started, { messageID: "msg_synthetic_shell", callID: "synthetic-shell", command: "synthetic command, not executed" })
          yield* publish(SessionEvent.Shell.Ended, { callID: "synthetic-shell", output: "Synthetic output" })
        }
        if (scenario === "compaction") {
          yield* publish(SessionEvent.Compaction.Started, { messageID: "msg_synthetic_compaction", reason: "manual" })
          yield* publish(SessionEvent.Compaction.Ended, { messageID: "msg_synthetic_compaction", reason: "manual", text: "Synthetic summary", recent: "Synthetic recent context" })
        }
      }
      const rows = yield* db.select().from(SessionMessageTable).all()
      const durable = yield* db.select().from(EventTable).all()
      if (scenario === "completed") {
        exported = durable.map((row) => ({ id: row.id, type: row.type, seq: row.seq, aggregateID: row.aggregate_id, data: row.data }))
      }
      const inputs = yield* db.select().from(SessionInputTable).all()
      yield* db.run("PRAGMA wal_checkpoint(TRUNCATE)")
      return {
        messageTypes: rows.map((row) => row.type),
        messageCount: rows.length,
        eventCount: durable.length,
        inputCount: inputs.length,
        assistantTokens: rows.filter((row) => row.type === "assistant").map((row) => row.data.tokens ?? null),
        assistantCompleted: rows.filter((row) => row.type === "assistant").map((row) => row.data.time.completed !== undefined),
      }
    }).pipe(Effect.provide(projected), Effect.scoped),
  )
}
await Bun.write("/fixture/producer-result.json", JSON.stringify(result, null, 2) + "\n")
