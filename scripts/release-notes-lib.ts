function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function extractChangelogSection(
  changelog: string,
  version: string,
): string | undefined {
  const lines = changelog.replace(/\r\n/g, "\n").split("\n");
  const headingPattern = new RegExp(
    `^##\\s+${escapeRegExp(version)}(?:\\s+[—-].+)?\\s*$`,
  );

  const start = lines.findIndex((line) => headingPattern.test(line));
  if (start === -1) return undefined;

  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^##\s+/.test(lines[index] ?? "")) {
      end = index;
      break;
    }
  }

  const section = lines.slice(start, end).join("\n").trim();
  return section.length > 0 ? `${section}\n` : undefined;
}

export function renderFallbackReleaseNotes(options: {
  readonly version: string;
  readonly commitSubjects: readonly string[];
  readonly previousTag?: string;
}): string {
  const { version, commitSubjects, previousTag } = options;
  const summary = previousTag
    ? `Changes since ${previousTag}.`
    : "Changes in this release.";
  const bullets = commitSubjects.length > 0
    ? commitSubjects.map((subject) => `- ${subject}`).join("\n")
    : "- Release metadata update.";

  return `## ${version}\n\n${summary}\n\n${bullets}\n`;
}

export function buildReleaseNotes(options: {
  readonly version: string;
  readonly changelog: string;
  readonly commitSubjects: readonly string[];
  readonly previousTag?: string;
}): string {
  const { version, changelog, commitSubjects, previousTag } = options;
  return extractChangelogSection(changelog, version)
    ?? renderFallbackReleaseNotes({ version, commitSubjects, previousTag });
}
