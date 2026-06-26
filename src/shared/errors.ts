export class MaestroError extends Error {
  readonly suggestions: readonly string[];

  constructor(message: string, suggestions: readonly string[] = []) {
    super(message);
    this.name = "MaestroError";
    this.suggestions = suggestions;
  }
}
