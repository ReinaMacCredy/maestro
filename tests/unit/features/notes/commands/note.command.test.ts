import { afterEach, describe, expect, it, mock } from "bun:test";
import { Command } from "commander";

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

async function loadRegisterNoteCommand(options: {
  readonly createNote?: (git: unknown, store: unknown, opts: { content: string; dir: string }) => Promise<unknown>;
  readonly listNotes?: (store: unknown) => Promise<unknown>;
}) {
  mock.module("@/services.js", () => ({
    getServices: () => ({
      git: { mocked: true },
      notesStore: { mocked: true },
    }),
  }));
  mock.module("@/features/notes/usecases/note.usecase.js", () => ({
    createNote: options.createNote ?? (async () => ({
      timestamp: "2026-04-15T09:00:00.000Z",
      git_branch: "main",
      content: "default",
    })),
    listNotes: options.listNotes ?? (async () => []),
  }));

  return import(`@/features/notes/commands/note.command.ts?test=${Date.now()}-${Math.random()}`);
}

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
  mock.restore();
});

describe("registerNoteCommand", () => {
  it("writes a note and formats text output", async () => {
    const captured = captureConsole();
    const { registerNoteCommand } = await loadRegisterNoteCommand({
      createNote: async (_git, _store, opts) => ({
        timestamp: "2026-04-15T09:00:00.000Z",
        git_branch: "feat/coverage",
        content: opts.content,
      }),
    });

    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerNoteCommand(program);

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
    const { registerNoteCommand } = await loadRegisterNoteCommand({
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
    registerNoteCommand(program);

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
    const { registerNoteCommand } = await loadRegisterNoteCommand({});
    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerNoteCommand(program);

    await expect(
      program.parseAsync(["node", "maestro", "note", "--list", "--content", "oops"]),
    ).rejects.toMatchObject({
      message: "--content and --list cannot be used together",
    });
  });

  it("rejects missing content when --list is not used", async () => {
    const { registerNoteCommand } = await loadRegisterNoteCommand({});
    const program = new Command().name("maestro").option("--json", "Output as JSON");
    registerNoteCommand(program);

    await expect(
      program.parseAsync(["node", "maestro", "note"]),
    ).rejects.toMatchObject({
      message: "--content is required unless --list is used",
    });
  });
});
