import type { ProductSpec, SpecMode, RiskClass } from "../types/product-spec.js";
import type { SpecStorePort } from "../repo/spec-store.port.js";
import { SpecAlreadyExistsError } from "../repo/spec-store.port.js";
import { isValidSpecSlug } from "../types/spec-id.js";

export interface SpecNewInput {
  readonly slug: string;
  readonly title?: string;
  readonly mode?: SpecMode;
  readonly riskClass?: RiskClass;
}

export interface SpecNewDeps {
  readonly store: SpecStorePort & {
    create?(spec: ProductSpec): Promise<void>;
  };
}

export class InvalidSpecSlugError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(
      `Invalid spec slug "${slug}": must be kebab-case ASCII, 3..64 chars, no leading or trailing hyphen, no consecutive hyphens`,
    );
    this.name = "InvalidSpecSlugError";
    this.slug = slug;
  }
}

const SKELETON_ACCEPTANCE = "Replace this line with a falsifiable acceptance criterion";
const SKELETON_NON_GOAL = "Replace this line with an explicit non-goal";

function buildSkeletonBody(title: string): string {
  return [
    `# ${title}`,
    "",
    "<!-- Run `maestro-design` grill protocol (ADR-0016) to refine acceptance",
    "     criteria, non-goals, risk class, and work type before claiming. -->",
    "",
    "## Context",
    "",
    "## Decisions",
    "",
  ].join("\n");
}

export async function specNew(deps: SpecNewDeps, input: SpecNewInput): Promise<ProductSpec> {
  if (!isValidSpecSlug(input.slug)) {
    throw new InvalidSpecSlugError(input.slug);
  }
  if (await deps.store.exists(input.slug)) {
    throw new SpecAlreadyExistsError(input.slug);
  }
  const title = input.title ?? input.slug;
  const spec: ProductSpec = {
    frontmatter: {
      slug: input.slug,
      acceptance_criteria: [SKELETON_ACCEPTANCE],
      non_goals: [SKELETON_NON_GOAL],
      risk_class: input.riskClass ?? "low",
      mode: input.mode ?? "light",
      work_type: "change-request",
    },
    body: buildSkeletonBody(title),
    path: `.maestro/specs/${input.slug}.md`,
  };
  await deps.store.write(spec);
  return spec;
}
