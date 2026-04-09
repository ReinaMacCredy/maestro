import type { NoteEntry } from "../domain/types.js";

export interface NotesStorePort {
  append(note: NoteEntry): Promise<void>;
  list(): Promise<readonly NoteEntry[]>;
}
