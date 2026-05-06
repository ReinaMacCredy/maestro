import { MaestroError } from "@/shared/errors.js";

// `--version` is reserved by Commander's root program (`.version(...)` in the
// CLI entry registers `-V/--version` globally). Subcommands that want to take
// a version-id parameter must use `--at-version` instead — using `--version`
// silently triggers Commander's print-binary-version-and-exit-0 path, which
// looks like the command worked. We catch and redirect three known cases.
//
// Match by collecting only positional (non-flag) words from the argv prefix,
// since `maestro contract show --task tsk-XXX --version 1` interleaves
// subcommand options between the verb and `--version`.
export function assertNoDeprecatedVersionFlag(argv: readonly string[]): void {
  const flagIdx = argv.findIndex((t) => t === "--version");
  if (flagIdx === -1) return;
  const next = argv[flagIdx + 1];
  if (typeof next !== "string" || next.startsWith("-")) return;

  const positional: string[] = [];
  let i = 2;
  while (i < flagIdx) {
    const token = argv[i] ?? "";
    if (token.startsWith("--")) {
      if (!token.includes("=")) {
        const follower = argv[i + 1];
        if (typeof follower === "string" && !follower.startsWith("-")) {
          i += 2;
          continue;
        }
      }
      i += 1;
      continue;
    }
    if (token.startsWith("-")) {
      i += 1;
      continue;
    }
    positional.push(token);
    i += 1;
  }
  const cmd = positional.join(" ");

  if (cmd === "verdict show" || cmd === "contract show") {
    throw new MaestroError(
      `\`${cmd} --version <id>\` collides with the global \`maestro --version\` flag`,
      [`Use \`--at-version ${next}\` instead (renamed in v0.72.18)`],
    );
  }
  if (cmd === "update") {
    throw new MaestroError(
      "`update --version <release>` collides with the global `maestro --version` flag",
      [`Use \`--release ${next}\` instead (renamed in v0.72.18)`],
    );
  }
}
