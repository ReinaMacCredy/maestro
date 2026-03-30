/**
 * Layout primitives -- Rect allocation for fixed-grid dashboard.
 */

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Split a rect horizontally by ratios (left to right).
 * Ratios are normalized: [1, 2] splits 33% / 67%.
 */
export function splitH(parent: Rect, ratios: number[]): Rect[] {
  if (ratios.length === 0) return [];
  const total = ratios.reduce((a, b) => a + b, 0);
  if (total === 0) return ratios.map(() => ({ ...parent, width: 0 }));

  const rects: Rect[] = [];
  let x = parent.x;
  let remaining = parent.width;

  for (let i = 0; i < ratios.length; i++) {
    const w = i === ratios.length - 1
      ? remaining
      : Math.round((ratios[i]! / total) * parent.width);
    rects.push({ x, y: parent.y, width: Math.max(0, w), height: parent.height });
    x += w;
    remaining -= w;
  }
  return rects;
}

/**
 * Split a rect vertically by fixed sizes (top to bottom).
 * Negative values mean "fill remaining". At most one negative allowed.
 */
export function splitV(parent: Rect, sizes: number[]): Rect[] {
  if (sizes.length === 0) return [];

  const flexIndex = sizes.findIndex((s) => s < 0);
  const fixedSum = sizes.reduce((a, s) => a + (s >= 0 ? s : 0), 0);
  const flexSize = Math.max(0, parent.height - fixedSum);

  const rects: Rect[] = [];
  let y = parent.y;

  for (let i = 0; i < sizes.length; i++) {
    const h = sizes[i]! < 0 ? flexSize : sizes[i]!;
    rects.push({ x: parent.x, y, width: parent.width, height: Math.max(0, h) });
    y += h;
  }
  return rects;
}

/** Shrink a rect inward by padding on all sides. */
export function inset(rect: Rect, padding: number): Rect {
  const p2 = padding * 2;
  return {
    x: rect.x + padding,
    y: rect.y + padding,
    width: Math.max(0, rect.width - p2),
    height: Math.max(0, rect.height - p2),
  };
}
