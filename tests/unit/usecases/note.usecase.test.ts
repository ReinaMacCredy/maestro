import { describe, expect, it } from "bun:test";
import { MaestroError } from "@/domain/errors.js";
import { createNote, listNotes } from "@/usecases/note.usecase.js";
import { mockGit, mockNotesStore } from "../../helpers/mocks.js";

describe("note usecase", () => {
  it("creates a timestamped note using the current git branch", async () => {
    const store = mockNotesStore();
    const note = await createNote(
      mockGit({
        getState: async () => ({
          branch: "refactor/rewrite-maestro",
          recentCommits: [],
          changedFiles: [],
          workingTreeClean: true,
          diffStat: "+0 -0",
        }),
      }),
      store,
      {
        content: "Remember the follow-up",
        dir: process.cwd(),
      },
    );

    expect(note.content).toBe("Remember the follow-up");
    expect(note.git_branch).toBe("refactor/rewrite-maestro");
    expect(note.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);

    const notes = await listNotes(store);
    expect(notes).toHaveLength(1);
    expect(notes[0]!.content).toBe("Remember the follow-up");
  });

  it("throws when run outside a git repository", async () => {
    const store = mockNotesStore();

    await expect(
      createNote(
        mockGit({
          isRepo: async () => false,
        }),
        store,
        {
          content: "No repo here",
          dir: "/tmp/not-a-repo",
        },
      ),
    ).rejects.toBeInstanceOf(MaestroError);
  });
});
