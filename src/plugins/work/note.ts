import {
  CliError,
  requiredPosition,
  type CliInvocation,
  type CliResult,
} from "../../kernel/cli.ts";
import { existsSync, readFileSync } from "node:fs";
import type { PluginContext } from "../../kernel/loader.ts";
import { registerSessionCommand } from "../session-required.ts";
import { requireWork } from "./records.ts";

export function registerWorkNote(context: PluginContext): void {
  context.effect(() =>
    registerSessionCommand(context, "work note", async (invocation): Promise<CliResult> => {
      // A long body travels as a file so it never meets the shell's quoting or
      // its command guards; both paths below read the body as positional 1.
      const file = invocation.options.file;
      if (typeof file === "string") {
        if (invocation.positionals[1] !== undefined) {
          throw new CliError("INVALID_OPTION", "--file replaces the note text; give one or the other");
        }
        if (!existsSync(file)) throw new CliError("NOT_FOUND", `note file not found: ${file}`, { file });
        invocation.positionals[1] = readFileSync(file, "utf8").trimEnd();
      }
      return context.events.waterfall<CliInvocation, CliResult>("work.note.invoke", invocation, noteWork);
    }, {
      description: "Append a note to a work item.",
      flags: {
        "--file": {
          description: "Read the note body from this file instead of the text argument.",
          value: true,
        },
      },
      positionals: [
        { name: "id", required: true },
        { name: "text", required: true },
      ],
    }),
  );

  function noteWork(invocation: CliInvocation): CliResult {
    const id = requiredPosition(invocation, 0, "work id");
    const text = requiredPosition(invocation, 1, "note text");
    requireWork(context, id);
    const createdAt = new Date().toISOString();
    context.store.database
      .query("INSERT INTO work_notes (work_id, text, created_at) VALUES (?, ?, ?)")
      .run(id, text, createdAt);
    context.log.append({
      type: "work.note",
      entityType: "work",
      entityId: id,
      sessionId: context.sessions.current().id,
      payload: { text },
    });
    return { data: { id, text }, text: `${id} note: ${text}` };
  }
}
