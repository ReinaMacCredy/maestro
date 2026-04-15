import { afterEach, describe, expect, it } from "bun:test";
import { Command } from "commander";
import type { CreateNoteOpts, NoteEntry, NotesStorePort } from "@/features/notes";
import { registerNoteCommand } from "@/features/notes";
import type { GitPort } from "@/infra/ports/git.port.js";

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

function captureConsole(): {
  readonly logs: string[];
  readonly errors: string[];
} {
  const logs: string[] = [];
  const errors: string[] = [];
  console.log = (...args: unknown[]) => {
    logs.push(args.map((arg) => String(arg)).join(" "));
  };
  console.error = (...args: unknown[]) => {
    errors.push(args.map((arg) => String(arg)).join(" "));
  };
  return { logs, errors };
}

function noteDeps(options: {
  readonly createNote?: (git: GitPort, store: NotesStorePort, opts: CreateNoteOpts) => Promise<NoteEntry>;
  readonly listNotes?: (store: NotesStorePort) => Promise<readonly NoteEntry[]>;
} = {}) {
  return {
    getServices: () => ({
      git: {
        isRepo: async () => true,
        getState: async () => ({
          branch: "main",
          recentCommits: [],
          changedFiles: [],
          workingTreeClean: true,
          diffStat: "+0 -0",
        }),
      },
      notesStore: {
        append: async () => undefined,
        list: async () => [],
      },
    }),
    createNote: options.createNote ?? (async () => ({
      timestamp: "2026-04-15T09:00:00.000Z",
      git_branch: "main",
      content: "default",
    })),
    listNotes: options.listNotes ?? (async () => []),
  };
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

describe("registerNoteCommand", () => {
    it("writes a note and formats text output", async () => {
      const captured = captureConsole();
      const deps = noteDeps({
        createNote: async (_git, _store, opts) => ({
          timestamp: "2026-04-15T09:00:00.000Z",
          git_branch: "feat/coverage",
          content: opts.content,
        }),
      });

      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerNoteCommand(program, deps);

    await program.parseAsync(["node", "maestro", "note", "--content", "remember this"]);

    expect(captured.logs).toEqual([
      "[ok] Note saved",
      "  Timestamp: 2026-04-15T09:00:00.000Z",
      "  Branch: feat/coverage",
      "  Content: remember this",
    ]);
  });

    it("lists notes in text mode and reports the empty state", async () => {
      const captured = captureConsole();
      let firstCall = true;
      const deps = noteDeps({
        listNotes: async () => {
          if (firstCall) {
            firstCall = false;
          return [
            {
              timestamp: "2026-04-15T09:00:00.000Z",
              git_branch: "main",
              content: "line\u001b[31m alert\u001b[0m",
            },
          ];
        }

          return [];
        },
      });

      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerNoteCommand(program, deps);

    await program.parseAsync(["node", "maestro", "note", "--list"]);
    expect(captured.logs).toEqual([
      "1 note(s)",
      "",
      "2026-04-15T09:00:00.000Z  [main]",
      "  line alert",
    ]);

    captured.logs.length = 0;

    await program.parseAsync(["node", "maestro", "note", "--list"]);
    expect(captured.logs).toEqual(["No notes found"]);
  });

    it("rejects using --content and --list together", async () => {
      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerNoteCommand(program, noteDeps());

    await expect(
      program.parseAsync(["node", "maestro", "note", "--list", "--content", "oops"]),
    ).rejects.toMatchObject({
      message: "--content and --list cannot be used together",
    });
  });

    it("rejects missing content when --list is not used", async () => {
      const program = new Command().name("maestro").option("--json", "Output as JSON");
      registerNoteCommand(program, noteDeps());

    await expect(
      program.parseAsync(["node", "maestro", "note"]),
    ).rejects.toMatchObject({
      message: "--content is required unless --list is used",
    });
  });
});
