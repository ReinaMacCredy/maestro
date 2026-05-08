// Escape so injected content cannot break out of the surrounding document structure.
export function escapeMarkdownBoundaries(content: string): string {
  return content
    .replace(/^(#{1,6})\s/gm, "\\$1 ")
    .replace(/^([>*+-])\s/gm, "\\$1 ")
    .replace(/^(\d+\.)\s/gm, "\\$1 ")
    .replace(/^```/gm, "\\```")
    .replace(/^\t+/gm, (match) => "\\t".repeat(match.length))
    .replace(/^( {4,})/gm, (match) => "\\" + match)
    .replace(/^(<!--)/gm, "\\$1")
    .replace(/^(--!?>)/gm, "\\$1");
}

function stripPromptMarkup(content: string): string {
  return content
    .replace(/<\/?system[^>]*>/gi, "")
    .replace(/<\/?instructions[^>]*>/gi, "")
    .replace(/<\/?user-prompt[^>]*>/gi, "")
    .replace(/<\/?assistant[^>]*>/gi, "");
}

function stripPromptControls(content: string): string {
  return content
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "")
    .replace(/\r\n?/g, "\n");
}

export function sanitizeInlinePromptContent(content: string): string {
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

  const collapsed = stripPromptControls(stripPromptMarkup(content))
    .split("\n")
    .map((line) => escapeMarkdownBoundaries(line.trim()))
    .filter((line) => line.length > 0)
    .join(" / ")
    .replace(/\s+/g, " ")
    .trim();

  return collapsed
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function sanitizeInlineCodeContent(content: string): string {
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

  return stripPromptControls(stripPromptMarkup(content))
    .split("\n")
    .map((line) => line.replace(/\t/g, "  "))
    .join(" / ");
}

export function sanitizePromptContent(content: string, label?: string): string {
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

  const tag = label ?? "user-content";

  let sanitized = stripPromptMarkup(content);
  sanitized = escapeMarkdownBoundaries(sanitized);
  sanitized = sanitized
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  return `<${tag}>\n${sanitized}\n</${tag}>`;
}

export function sanitizeTerminalText(content: string | undefined): string {
  if (!content) {
    return "";
  }

  return content
    // OSC sequences
    .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/g, "")
    // CSI sequences
      .replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
      // Other escape-led control sequences
      .replace(/\u001b[@-_]/g, "")
      // Preserve layout spacing for whitespace controls only
      .replace(/[\u0009-\u000D]/g, " ")
      // Strip remaining control characters
      .replace(/[\u0000-\u0008\u000E-\u001F\u007F]/g, "");
}
