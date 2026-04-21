export function normalizeSlashes(path: string): string {
  return path.replaceAll("\\", "/");
}
