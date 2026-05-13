import type { Command } from "commander";
import type { NoteEntry } from "../domain/types.js";
import { MaestroError } from "@/shared/errors.js";
import { output } from "@/shared/lib/output.js";
import { type Services } from "@/services.js";
import { createNote as defaultCreateNote, listNotes as defaultListNotes } from "../usecases/note.usecase.js";

interface NoteCommandDeps {
  readonly getServices: () => Pick<Services, "git" | "notesStore">;
  readonly createNote?: typeof defaultCreateNote;
  readonly listNotes?: typeof defaultListNotes;
}

export function registerNoteCommand(
  program: Command,
  deps: NoteCommandDeps,
): void {
  program
    .command("note [text...]")
    .description("Append or list project notes")
    .addHelpText("after", `
Examples:
  maestro note "Remember to rerun doctor after init"
  maestro note --content "Same as above, explicit flag"
  maestro note --list
  maestro note --list --json
`)
    .option("--content <text>", "Note content to append")
    .option("--list", "List saved notes")
    .option("--json", "Output as JSON")
    .action(async (textParts: string[], opts): Promise<void> => {
        const services = deps.getServices();
        const isJson = opts.json ?? program.opts().json;
        const positional = textParts.length > 0 ? textParts.join(" ") : undefined;

      if (positional !== undefined && opts.content !== undefined) {
        throw new MaestroError("Pass the note text positionally or via --content, not both", [
          "maestro note \"text\"",
          "maestro note --content \"text\"",
        ]);
      }
      const content = positional ?? opts.content;

      if (opts.list && content !== undefined) {
        throw new MaestroError("--content and --list cannot be used together", [
          "maestro note --content '...'",
          "maestro note --list",
        ]);
      }

        if (opts.list) {
          const notes = await (deps.listNotes ?? defaultListNotes)(services.notesStore);
          output(isJson, notes, formatList);
          return;
        }

      if (content === undefined) {
        throw new MaestroError("Provide the note text positionally or via --content, or pass --list", [
          "maestro note \"text\"",
          "maestro note --content '...'",
          "maestro note --list",
        ]);
      }

        const note = await (deps.createNote ?? defaultCreateNote)(services.git, services.notesStore, {
          content,
          dir: process.cwd(),
        });

      output(isJson, note, formatSaved);
    });
}

function formatSaved(note: NoteEntry): string[] {
  return [
    "[ok] Note saved",
    `  Timestamp: ${note.timestamp}`,
    `  Branch: ${note.git_branch}`,
    `  Content: ${note.content}`,
  ];
}

function formatList(notes: readonly NoteEntry[]): string[] {
  if (notes.length === 0) {
    return ["No notes found"];
  }

  const lines: string[] = [`${notes.length} note(s)`];
  for (const note of notes) {
    lines.push(
      "",
      `${note.timestamp}  [${note.git_branch}]`,
      `  ${note.content}`,
    );
  }
  return lines;
}
