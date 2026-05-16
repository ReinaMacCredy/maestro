import type { ProductSpec } from "../types/product-spec.js";

export interface SpecStorePort {
  read(slug: string): Promise<ProductSpec>;
  write(spec: ProductSpec): Promise<void>;
  exists(slug: string): Promise<boolean>;
  list(): Promise<readonly string[]>;
}

export class SpecParseError extends Error {
  readonly field?: string;
  constructor(message: string, field?: string) {
    super(message);
    this.name = "SpecParseError";
    this.field = field;
  }
}

export class SpecNotFoundError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Product spec ${slug} not found`);
    this.name = "SpecNotFoundError";
    this.slug = slug;
  }
}

export class SpecAlreadyExistsError extends Error {
  readonly slug: string;
  constructor(slug: string) {
    super(`Product spec ${slug} already exists`);
    this.name = "SpecAlreadyExistsError";
    this.slug = slug;
  }
}
