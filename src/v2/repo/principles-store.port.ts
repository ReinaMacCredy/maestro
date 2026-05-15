import type { Principle } from "../types/principle.js";

export interface PrinciplesStorePort {
  list(): Promise<readonly Principle[]>;
  get(slug: string): Promise<Principle | undefined>;
  exists(slug: string): Promise<boolean>;
  write(slug: string, content: string): Promise<void>;
}

export class PrinciplesNotFoundError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Principle ${slug} not found`);
    this.name = "PrinciplesNotFoundError";
    this.slug = slug;
  }
}

export class PrincipleAlreadyExistsError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Principle ${slug} already exists`);
    this.name = "PrincipleAlreadyExistsError";
    this.slug = slug;
  }
}
