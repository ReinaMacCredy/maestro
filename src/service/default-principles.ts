// Default principles pack written by the `seed-principles` step of
// `maestro setup` when docs/principles/ is empty. Each entry maps to
// docs/principles/<slug>.md. Markdown bodies live in
// @/shared/data/default-principle-content so the passive-harness lint
// (scoped to layered src/) does not match the token names embedded in
// the rule prose.

import {
  LAYER_ORDER_MD,
  NO_YOLO_DATA_PROBING_MD,
  PASSIVE_HARNESS_MD,
  PREFER_SHARED_UTILS_MD,
} from "@/shared/data/default-principle-content.js";

export interface DefaultPrinciple {
  readonly slug: string;
  readonly content: string;
}

export const DEFAULT_PRINCIPLES: readonly DefaultPrinciple[] = [
  { slug: "layer-order", content: LAYER_ORDER_MD },
  { slug: "no-yolo-data-probing", content: NO_YOLO_DATA_PROBING_MD },
  { slug: "passive-harness", content: PASSIVE_HARNESS_MD },
  { slug: "prefer-shared-utils", content: PREFER_SHARED_UTILS_MD },
];
