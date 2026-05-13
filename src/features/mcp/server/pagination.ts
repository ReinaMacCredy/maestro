export const DEFAULT_PAGE_SIZE = 20;
export const MAX_PAGE_SIZE = 100;

export interface PaginationMeta {
  readonly total: number;
  readonly limit: number;
  readonly offset: number;
  readonly hasMore: boolean;
}

export interface PageResult<T = unknown> {
  readonly items: readonly T[];
  readonly pagination: PaginationMeta;
}

export function paginate<T = unknown>(
  items: readonly T[],
  limit: number | undefined,
  offset: number | undefined,
): PageResult<T> {
  const clampedLimit = Math.min(Math.max(1, limit ?? DEFAULT_PAGE_SIZE), MAX_PAGE_SIZE);
  const clampedOffset = Math.max(offset ?? 0, 0);
  const slice = items.slice(clampedOffset, clampedOffset + clampedLimit);
  return {
    items: slice,
    pagination: {
      total: items.length,
      limit: clampedLimit,
      offset: clampedOffset,
      hasMore: clampedOffset + clampedLimit < items.length,
    },
  };
}
