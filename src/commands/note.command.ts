import type { Command } from "commander";
import type { NoteEntry } from "../domain/types.js";
import { output } from "../lib/output.js";
import { getServices } from "../services.js";
import { createNote, listNotes } from "../usecases/note.usecase.js";

export function registerNoteCommand(program: Command): void {
  program
    .command("note")
    .description("Append or list project notes")
    .addHelpText("after", `
Examples:
  maestro note --content "Remember to rerun doctor after init"
  maestro note --list
  maestro note --list --json
`)
    .option("--content <text>", "Note content to append")
    .option("--list", "List saved notes")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      const services = getServices();
      const isJson = opts.json ?? program.opts().json;

      if (opts.list && opts.content) {
        console.error("[!] --content and --list cannot be used together");
        console.error("    maestro note --content '...'");
        console.error("    maestro note --list");
        process.exit(1);
      }

      if (opts.list) {
        const notes = await listNotes(services.notesStore);
        output(isJson, notes, formatList);
        return;
      }

      if (!opts.content) {
        console.error("[!] --content is required unless --list is used");
        console.error("    maestro note --content '...'");
        console.error("    maestro note --list");
        process.exit(1);
      }

      const note = await createNote(services.git, services.notesStore, {
        content: opts.content,
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
