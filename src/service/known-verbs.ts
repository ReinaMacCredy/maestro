import type { Command } from "commander";

// Verbs that are registered lazily (after the parity check runs) and therefore
// must be added explicitly. Mission Control is the canonical case: its OpenTUI
// graph costs ~250ms cold, so the registration is deferred behind argv parsing.
const LAZY_VERBS: readonly string[] = ["mission-control"];

/**
 * Walk a Commander program tree and return the set of verbs the binary
 * actually exposes. Both leaf names (e.g. `claim`) and full paths
 * (e.g. `task claim`) are included so the parity check can validate either
 * shape a skill might cite.
 *
 * Shared between `maybePrintSkillDriftHint` in src/index.ts and the
 * `checkSkillBinaryParity` test suite so they cannot drift apart.
 */
export function collectKnownVerbs(program: Command): Set<string> {
  const verbs = new Set<string>();
  const walk = (cmd: Command, prefix: string): void => {
    for (const child of cmd.commands) {
      const name = child.name();
      const full = prefix ? `${prefix} ${name}` : name;
      verbs.add(name);
      verbs.add(full);
      walk(child, full);
    }
  };
  walk(program, "");
  for (const lazy of LAZY_VERBS) verbs.add(lazy);
  return verbs;
}
