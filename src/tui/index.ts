/**
 * TUI public API -- barrel re-exports.
 */
export { renderDashboard, type InteractiveOptions } from "./app/app.js";
export { renderOnceFrame, renderFrame, type OnceFrameOptions } from "./app/render.js";
export { keyToAction } from "./app/input-dispatch.js";
export type { Action } from "./state.js";
