export class MaestroError extends Error {
  readonly hints: readonly string[];
  readonly code?: string;

  constructor(message: string, hints: readonly string[] = [], code?: string) {
    super(message);
    this.name = "MaestroError";
    this.hints = hints;
    this.code = code;
  }
}
