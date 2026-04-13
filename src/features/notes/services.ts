import type { NotesStorePort } from "./ports/notes-store.port.js";
import { FsNotesStoreAdapter } from "./adapters/notes-store.adapter.js";

export interface NotesServices {
  readonly notesStore: NotesStorePort;
}

export function buildNotesServices(projectDir: string): NotesServices {
  return {
    notesStore: new FsNotesStoreAdapter(projectDir),
  };
}
