/** Minimal template renderer with {{var}} substitution and {{#var}}...{{/var}} conditionals. */
export function renderTemplate(
  template: string,
  vars: Record<string, string>,
): string {
  // Process conditionals: {{#var}}...{{/var}} blocks render only if var is truthy
  let result = template.replace(
    /\{\{#(\w+)\}\}([\s\S]*?)\{\{\/\1\}\}/g,
    (_, key, content) => (vars[key] ? content : ""),
  );

  // Substitute variables: {{var}}
  result = result.replace(
    /\{\{(\w+)\}\}/g,
    (_, key) => vars[key] ?? "",
  );

  return result;
}
