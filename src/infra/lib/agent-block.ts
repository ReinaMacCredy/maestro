import { BLOCK_START_MARKER, BLOCK_END_MARKER, REFERENCE_FILE } from "../domain/agents.js";

const BLOCK_REGEX = new RegExp(
  `${BLOCK_START_MARKER.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[\\s\\S]*?${BLOCK_END_MARKER.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`,
);
const LEGACY_HEADING_REGEX = /\n## Cross-Agent Handoff \(maestro\)[\s\S]*?(?=\n## |\n$|$)/;

const REFERENCE_LINE = `@${REFERENCE_FILE}`;

function referenceRegex(): RegExp {
  return new RegExp(`^${REFERENCE_LINE.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*$`, "m");
}

export function hasReference(content: string): boolean {
  return referenceRegex().test(content);
}

export function injectReference(content: string): string {
  if (hasReference(content)) return content;
  const trimmed = content.trimEnd();
  if (trimmed.length === 0) return REFERENCE_LINE + "\n";
  return trimmed + "\n\n" + REFERENCE_LINE + "\n";
}

export function removeReference(content: string): string | null {
  if (!hasReference(content)) return null;
  return content
    .replace(referenceRegex(), "")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd() + "\n";
}

export function wrapBlock(content: string): string {
  return `${BLOCK_START_MARKER}\n${content}\n${BLOCK_END_MARKER}`;
}

export function hasBlock(content: string): boolean {
  return BLOCK_REGEX.test(content);
}

export function extractBlock(content: string): string | null {
  const match = content.match(BLOCK_REGEX);
  if (!match) return null;
  return match[0]
    .replace(BLOCK_START_MARKER, "")
    .replace(BLOCK_END_MARKER, "")
    .trim();
}

export function injectBlock(content: string, block: string): string {
  const wrapped = wrapBlock(block);
  const trimmed = content.trimEnd();
  if (trimmed.length === 0) return wrapped + "\n";
  return trimmed + "\n\n" + wrapped + "\n";
}

export function replaceBlock(content: string, newBlock: string): string | null {
  if (!hasBlock(content)) return null;
  return content.replace(BLOCK_REGEX, wrapBlock(newBlock));
}

export function removeBlock(content: string): string | null {
  if (!hasBlock(content)) return null;
  return content
    .replace(BLOCK_REGEX, "")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd() + "\n";
}

export function removeLegacyBlock(content: string): string | null {
  // Don't touch marked blocks -- only remove unmarked legacy sections
  if (hasBlock(content)) return null;
  if (!LEGACY_HEADING_REGEX.test(content)) return null;
  return content
    .replace(LEGACY_HEADING_REGEX, "")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd() + "\n";
}
