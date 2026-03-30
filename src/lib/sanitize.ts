/**
 * Sanitize user-provided content for inclusion in agent prompts.
 * Wraps content in XML delimiters and strips known injection patterns.
 */
export function sanitizePromptContent(content: string, label?: string): string {
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

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

  // Wrap in XML delimiters
  const tag = label ?? "user-content";
  return `<${tag}>\n${sanitized}\n</${tag}>`;
}
