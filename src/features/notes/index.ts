/**
 * Public surface for the notes feature.
 *
 * Cross-feature consumers (composition root, tests) import from
 * `@/features/notes`. Deep paths into the feature are not allowed from
 * outside (enforced by `bun run check:boundaries`).
 */
export type { NoteEntry } from "./domain/types.js";
export type { NotesStorePort } from "./ports/notes-store.port.js";
export { FsNotesStoreAdapter } from "./adapters/notes-store.adapter.js";
export { createNote, listNotes, type CreateNoteOpts } from "./usecases/note.usecase.js";
export { registerNoteCommand } from "./commands/note.command.js";
