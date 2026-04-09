import { join } from "node:path";
import type { NoteEntry } from "@/domain/types.js";
import { MAESTRO_DIR } from "@/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/lib/fs.js";
import type { NotesStorePort } from "../ports/notes-store.port.js";

export class FsNotesStoreAdapter implements NotesStorePort {
  constructor(private readonly baseDir: string) {}

  private notesPath(): string {
    return join(this.baseDir, MAESTRO_DIR, "notes.json");
  }

  async append(note: NoteEntry): Promise<void> {
    await ensureDir(join(this.baseDir, MAESTRO_DIR));
    const notes = await this.readAll();
    notes.push(note);
    await writeJson(this.notesPath(), notes);
  }

  async list(): Promise<readonly NoteEntry[]> {
    return this.readAll();
  }

  private async readAll(): Promise<NoteEntry[]> {
    return (await readJson<NoteEntry[]>(this.notesPath())) ?? [];
  }
}
