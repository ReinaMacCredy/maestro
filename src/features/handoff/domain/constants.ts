/**
 * Handoff-scoped constants.
 *
 * `NO_SESSION_ID` is the sentinel used by `create-uki-handoff.usecase.ts`
 * and `handoff-store.adapter.ts` when session detection is unavailable.
 * It moved here from `src/domain/defaults.ts` in Phase 2 of the
 * feature-folder migration because it is handoff-local state.
 */
export const NO_SESSION_ID = "none";
