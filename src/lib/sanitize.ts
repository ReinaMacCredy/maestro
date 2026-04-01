/**
 * Sanitize user-provided content for inclusion in agent prompts.
 * Wraps content in XML delimiters and strips known injection patterns.
 */
export function sanitizePromptContent(content: string, label?: string): string {
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

  const tag = label ?? "user-content";

  // Strip known injection patterns
  let sanitized = content
    .replace(/<\/?system[^>]*>/gi, "")
    .replace(/<\/?instructions[^>]*>/gi, "")
    .replace(/<\/?user-prompt[^>]*>/gi, "")
    .replace(/<\/?assistant[^>]*>/gi, "");

  // Escape markdown headers and HTML comments
  sanitized = sanitized
    .replace(/^(#{1,6})\s/gm, "\\$1 ")
    .replace(/^(<!--)/gm, "\\$1")
    .replace(/^(-->)/gm, "\\$1");

  // Encode markup characters so user content remains literal within the wrapper.
  sanitized = sanitized
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // Wrap in XML delimiters
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
    // Remaining control characters
    .replace(/[\u0000-\u001F\u007F]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
