import {
  BLOCK_START_MARKER,
  BLOCK_END_MARKER,
  REFERENCE_FILE,
  SETUP_BLOCK_START_MARKER,
  SETUP_BLOCK_END_MARKER,
  SETUP_REFERENCE_FILE,
} from "../domain/agents.js";
import { escapeRegex } from "@/shared/lib/regex.js";

function buildBlockRegex(startMarker: string, endMarker: string): RegExp {
  return new RegExp(`${escapeRegex(startMarker)}[\\s\\S]*?${escapeRegex(endMarker)}`);
}

function buildReferenceRegex(line: string): RegExp {
  return new RegExp(`^${escapeRegex(line)}\\s*$`, "m");
}

const BLOCK_REGEX = buildBlockRegex(BLOCK_START_MARKER, BLOCK_END_MARKER);
const SETUP_BLOCK_REGEX = buildBlockRegex(SETUP_BLOCK_START_MARKER, SETUP_BLOCK_END_MARKER);
const LEGACY_HEADING_REGEX = /\n## Cross-Agent Handoff \(maestro\)[\s\S]*?(?=\n## |\n$|$)/;

const REFERENCE_LINE = `@${REFERENCE_FILE}`;
const SETUP_REFERENCE_LINE = `@${SETUP_REFERENCE_FILE}`;
const REFERENCE_REGEX = buildReferenceRegex(REFERENCE_LINE);
const SETUP_REFERENCE_REGEX = buildReferenceRegex(SETUP_REFERENCE_LINE);

function wrapWithMarkers(content: string, startMarker: string, endMarker: string): string {
  return `${startMarker}\n${content}\n${endMarker}`;
}

function injectWrappedBlock(content: string, wrapped: string): string {
  const trimmed = content.trimEnd();
  if (trimmed.length === 0) return wrapped + "\n";
  return trimmed + "\n\n" + wrapped + "\n";
}

function appendReferenceIfMissing(content: string, line: string, regex: RegExp): string {
  if (regex.test(content)) return content;
  const trimmed = content.trimEnd();
  if (trimmed.length === 0) return line + "\n";
  return trimmed + "\n\n" + line + "\n";
}

function stripReferenceIfPresent(content: string, regex: RegExp): string | null {
  if (!regex.test(content)) return null;
  return content
    .replace(regex, "")
    .replace(/\n{3,}/g, "\n\n")
    .trimEnd() + "\n";
}

export function hasReference(content: string): boolean {
  return REFERENCE_REGEX.test(content);
}

export function injectReference(content: string): string {
  return appendReferenceIfMissing(content, REFERENCE_LINE, REFERENCE_REGEX);
}

export function removeReference(content: string): string | null {
  return stripReferenceIfPresent(content, REFERENCE_REGEX);
}

export function hasSetupReference(content: string): boolean {
  return SETUP_REFERENCE_REGEX.test(content);
}

export function injectSetupReference(content: string): string {
  return appendReferenceIfMissing(content, SETUP_REFERENCE_LINE, SETUP_REFERENCE_REGEX);
}

export function wrapBlock(content: string): string {
  return wrapWithMarkers(content, BLOCK_START_MARKER, BLOCK_END_MARKER);
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
  return injectWrappedBlock(content, wrapBlock(block));
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

function wrapSetupBlock(content: string): string {
  return wrapWithMarkers(content, SETUP_BLOCK_START_MARKER, SETUP_BLOCK_END_MARKER);
}

export function hasSetupBlock(content: string): boolean {
  return SETUP_BLOCK_REGEX.test(content);
}

export function injectSetupBlock(content: string, block: string): string {
  return injectWrappedBlock(content, wrapSetupBlock(block));
}

export function replaceSetupBlock(content: string, newBlock: string): string | null {
  if (!hasSetupBlock(content)) return null;
  return content.replace(SETUP_BLOCK_REGEX, wrapSetupBlock(newBlock));
}
