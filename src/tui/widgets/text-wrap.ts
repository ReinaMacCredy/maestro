/**
 * Text wrapping and truncation utilities.
 */

/**
 * Word-wrap text to fit within maxWidth. Returns array of lines.
 * If more lines than maxLines, truncates with "+N more" suffix.
 */
export function wrapText(text: string, maxWidth: number, maxLines = Infinity): string[] {
  if (maxWidth <= 0) return [];
  if (!text) return [];

  const words = text.split(/\s+/);
  const lines: string[] = [];
  let currentLine = "";

  for (const word of words) {
    if (!word) continue;

    if (currentLine.length === 0) {
      currentLine = word.length > maxWidth ? word.slice(0, maxWidth) : word;
    } else if (currentLine.length + 1 + word.length <= maxWidth) {
      currentLine += " " + word;
    } else {
      lines.push(currentLine);
      currentLine = word.length > maxWidth ? word.slice(0, maxWidth) : word;
    }
  }

  if (currentLine.length > 0) {
    lines.push(currentLine);
  }

  if (lines.length <= maxLines) return lines;

  const truncated = lines.slice(0, maxLines - 1);
  const remaining = lines.length - (maxLines - 1);
  truncated.push(`+${remaining} more`);
  return truncated;
}
