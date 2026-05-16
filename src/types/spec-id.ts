// Spec slug rules: kebab-case, ASCII lowercase letters/digits and hyphens,
// no leading or trailing hyphen, no consecutive hyphens, 3..64 chars.

export const SPEC_SLUG_PATTERN = /^[a-z0-9](?:[a-z0-9]|-(?=[a-z0-9])){2,63}$/;

export function isValidSpecSlug(value: unknown): value is string {
  return typeof value === "string" && SPEC_SLUG_PATTERN.test(value);
}

export function generateSpecSlug(title: string): string {
  const slug = title
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9\s-]+/g, "")
    .trim()
    .replace(/[\s-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64);
  return slug.replace(/-+$/g, "");
}
