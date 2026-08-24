import type { BuiltInPlugin, PluginContext } from "../kernel/loader.ts";
import type { WorkRecord, WorkService } from "./work.ts";

interface StartInput {
  children: WorkRecord[];
  sessionId: string;
  work: WorkRecord;
}

interface NoteRow {
  text: string;
}

function hasResearchNote(context: PluginContext, id: string): boolean {
  return context.store.database
    .query<NoteRow, [string]>("SELECT text FROM work_notes WHERE work_id = ? ORDER BY id")
    .all(id)
    .some((note) => note.text.startsWith("research:"));
}

export const policyResearchPlugin: BuiltInPlugin = {
  name: "policy-research",
  defaultDisabled: true,
  inject: ["work"],
  requires:
    'gates work start on parentless features: requires a "research: <finding>" note or a done research child',
  apply(context) {
    const work = context.work as WorkService;
    context.effect(() =>
      context.events.on<StartInput>("work.start", async (input, next) => {
        if (input.work.parentId || input.work.kind !== "feature") return next();
        const completedResearch = work
          .children(input.work.id)
          .some((child) => child.kind === "research" && child.state === "done");
        if (hasResearchNote(context, input.work.id) || completedResearch) return next();
        return {
          blocked: true,
          origin: "policy-research",
          reason:
            `parentless feature work requires prior research; run: maestro work note ` +
            `${input.work.id} "research: <finding and source>"`,
        };
      }),
    );
  },
};
