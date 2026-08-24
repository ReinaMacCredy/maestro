import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { WorkRecord, WorkService } from "./work.ts";

interface CompletionInput {
  claims: string[];
  evidence: string;
  proofs: string[];
  work: WorkRecord;
}

interface CompletionResult {
  blocked: boolean;
  evidence?: string;
  origin?: string;
  reason?: string;
}

interface NoteEventRow {
  payload: string;
  session_id: string | null;
}

function hasIndependentWitness(
  context: PluginContext,
  workId: string,
  completingSession: string,
): boolean {
  return context.store.database
    .query<NoteEventRow, [string]>(
      `SELECT session_id, payload FROM event_log
       WHERE type = 'work.note' AND entity_id = ? ORDER BY id`,
    )
    .all(workId)
    .some((event) => {
      if (!event.session_id || event.session_id === completingSession) return false;
      const payload = JSON.parse(event.payload) as { text?: unknown };
      return typeof payload.text === "string" && payload.text.startsWith("witness:");
    });
}

export const policyWitnessPlugin: BuiltInPlugin = {
  name: "policy-witness",
  defaultDisabled: true,
  inject: ["work"],
  requires:
    'gates work done on parents with children: requires a "witness: <finding>" note from a different session',
  apply(context) {
    const work = context.work as WorkService;
    context.effect(() =>
      context.events.on<CompletionInput, CompletionResult>("work.done", async (input, next) => {
        if (work.children(input.work.id).length === 0) return next();
        const sessionId = context.sessions.current().id;
        if (hasIndependentWitness(context, input.work.id, sessionId)) return next();
        return {
          blocked: true,
          origin: "policy-witness",
          reason:
            `parent completion requires an independent witness note; from a different session run: ` +
            `maestro work note ${input.work.id} "witness: <independent finding>"`,
        };
      }),
    );
  },
};
