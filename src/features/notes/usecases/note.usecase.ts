import type { NoteEntry } from "@/domain/types.js";
import { MaestroError } from "@/shared/errors.js";
import type { GitPort } from "@/ports/git.port.js";
import type { NotesStorePort } from "../ports/notes-store.port.js";

export interface CreateNoteOpts {
  readonly content: string;
  readonly dir: string;
}

export async function createNote(
  git: GitPort,
  store: NotesStorePort,
  opts: CreateNoteOpts,
): Promise<NoteEntry> {
  if (!(await git.isRepo(opts.dir))) {
    throw new MaestroError("Not a git repository", [
      "Run this command from inside a git repo",
    ]);
  }

  const gitState = await git.getState(opts.dir);
  const note: NoteEntry = {
    timestamp: new Date().toISOString(),
    content: opts.content,
    git_branch: gitState.branch,
  };

  await store.append(note);
  return note;
}

export async function listNotes(
  store: NotesStorePort,
): Promise<readonly NoteEntry[]> {
  return store.list();
}
