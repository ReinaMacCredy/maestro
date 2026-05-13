import { isManagedSkillDirectoryName } from "./skill-path.js";

const SUBSTRATE_PREFIXES = [".maestro/"] as const;
const SUBSTRATE_EXACT = [".maestro"] as const;
const MANAGED_AGENT_SKILL_ROOT_PREFIXES = [".claude/skills/", ".codex/skills/"] as const;

/**
 * True for any path maestro itself owns and writes during the project
 * lifecycle. These paths must be exempt from contract scope checks (both
 * the Trust Verifier's `check-scope` and the close-path verdict): they
 * appear in the diff between lock-commit and HEAD by construction (e.g.
 * `maestro init` writes them, the heartbeat refreshes NOW.md), so gating
 * them with the user's `filesExpected` scope produces false positives
 * that close otherwise-clean contracts as `broken`.
 *
 * Today three categories qualify:
 *   - `.maestro/`       — task/contract/evidence/run state
 *   - `.claude/skills/maestro:<name>/` — bundled Claude skills shipped by `maestro init`
 *   - `.codex/skills/maestro:<name>/`  — bundled Codex skills shipped by `maestro init`
 *
 * Skill directories are URL-encoded on disk (`maestro:` → `maestro%3A`);
 * `isManagedSkillDirectoryName` recognises both spellings, so tests and
 * tools can use either form without coordinating on filesystem encoding.
 *
 * Non-prefix matches (e.g. `.claude/skills/my-team/SKILL.md`) are NOT
 * exempt: only the maestro-prefixed bundles are substrate. A
 * project-authored skill outside the `maestro:` namespace is user code
 * and must be declared in the contract scope like any other file.
 */
export function isMaestroSubstratePath(path: string): boolean {
  for (const exact of SUBSTRATE_EXACT) {
    if (path === exact) return true;
  }
  for (const prefix of SUBSTRATE_PREFIXES) {
    if (path.startsWith(prefix)) return true;
  }
  for (const root of MANAGED_AGENT_SKILL_ROOT_PREFIXES) {
    if (!path.startsWith(root)) continue;
    const rest = path.slice(root.length);
    const slashIndex = rest.indexOf("/");
    const skillDir = slashIndex === -1 ? rest : rest.slice(0, slashIndex);
    if (isManagedSkillDirectoryName(skillDir)) return true;
  }
  return false;
}
