export type { NoteEntry } from "./domain/types.js";
export type { NotesStorePort } from "./ports/notes-store.port.js";
export { FsNotesStoreAdapter } from "./adapters/notes-store.adapter.js";
export { createNote, listNotes, type CreateNoteOpts } from "./usecases/note.usecase.js";
export { registerNoteCommand } from "./commands/note.command.js";
export { buildNotesServices } from "./services.js";
export type { NotesServices } from "./services.js";
