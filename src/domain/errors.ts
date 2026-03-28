export class MaestroError extends Error {
  readonly hints: readonly string[];

  constructor(message: string, hints: readonly string[] = []) {
    super(message);
    this.name = "MaestroError";
    this.hints = hints;
  }
}

export function handoffNotFound(id: string): MaestroError {
  return new MaestroError(`Handoff ${id} not found`, [
    "List handoffs: maestro handoff --list",
  ]);
}
