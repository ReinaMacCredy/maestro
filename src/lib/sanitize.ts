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

  // Prevent user content from closing or nesting the wrapper tag.
  const escapedTag = tag.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  sanitized = sanitized
    .replace(new RegExp(`<${escapedTag}>`, "gi"), `&lt;${tag}&gt;`)
    .replace(new RegExp(`</${escapedTag}>`, "gi"), `&lt;/${tag}&gt;`);

  // Wrap in XML delimiters
  return `<${tag}>\n${sanitized}\n</${tag}>`;
}
