/**
 * Auto-infer memory metadata (tags, category) from content and filename.
 * Solves the bootstrap problem: memories without explicit frontmatter
 * still get useful metadata for DCP scoring.
 */

import type { MemoryMetadata, MemoryCategory } from './types.ts';

interface CategoryRule {
  keywords: string[];
  category: MemoryCategory;
}

const CATEGORY_RULES: CategoryRule[] = [
  { keywords: ['decided', 'chose', 'rejected', 'trade-off'], category: 'decision' },
  { keywords: ['found that', 'discovered', 'investigated', 'compared'], category: 'research' },
  { keywords: ['architecture', 'design pattern', 'component', 'layer'], category: 'architecture' },
  { keywords: ['convention', 'standard', 'guideline', 'always use'], category: 'convention' },
  { keywords: ['bug', 'error', 'fix', 'workaround', 'debug'], category: 'debug' },
];

function inferCategory(bodyContent: string): MemoryCategory {
  const lower = bodyContent.toLowerCase();
  for (const rule of CATEGORY_RULES) {
    if (rule.keywords.some(kw => lower.includes(kw))) {
      return rule.category;
    }
  }
  return 'research';
}

const HEADING_RE = /^#+\s+(.+)$/gm;

function inferTags(bodyContent: string, fileName: string): string[] {
  const tags = new Set<string>();

  // Extract from filename (split on - and _)
  const nameWords = fileName.replace(/\.md$/, '').split(/[-_]+/);
  for (const word of nameWords) {
    if (word.length >= 4) tags.add(word.toLowerCase());
  }

  // Extract from headings
  HEADING_RE.lastIndex = 0; // reset stateful /g regex between calls
  let match: RegExpExecArray | null;
  while ((match = HEADING_RE.exec(bodyContent)) !== null) {
    const headingWords = match[1].split(/\s+/);
    for (const word of headingWords) {
      const clean = word.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
      if (clean.length >= 4) tags.add(clean);
    }
  }

  // Return top 5 deduplicated
  return [...tags].slice(0, 5);
}

/**
 * Infer metadata for a memory file when not explicitly provided.
 */
export function inferMetadata(bodyContent: string, fileName: string): MemoryMetadata {
  return {
    tags: inferTags(bodyContent, fileName),
    priority: 2,
    category: inferCategory(bodyContent),
  };
}
