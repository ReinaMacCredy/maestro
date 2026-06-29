---
source: maestro
style: neutral
tokens:
  color:
    background: "#f8fafc"
    foreground: "#111827"
    surface: "#ffffff"
    muted: "#64748b"
    accent: "#2563eb"
    success: "#047857"
    warning: "#b45309"
    danger: "#b91c1c"
  radius:
    control: 6
    card: 8
  typography:
    body: "system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"
    mono: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
---

# DESIGN.md

## Product Posture

Build the interface as a working product surface, not a marketing placeholder.
Prefer dense, calm layouts for operational tools and direct manipulation for
creative tools. The first screen should expose the primary workflow.

## Layout

- Use responsive grids, sidebars, tabs, and toolbars that keep controls close to
  the work area.
- Keep repeated item cards at 8px radius or less unless the local design system
  says otherwise.
- Avoid nested cards and decorative section cards.
- Define stable dimensions for boards, tiles, canvases, toolbars, icon buttons,
  and counters so hover states and dynamic labels do not shift the layout.

## Visual System

- Use a restrained neutral base with one clear accent and semantic states for
  success, warning, and danger.
- Avoid one-note palettes dominated by a single hue family.
- Use icons for compact actions when a familiar symbol exists; pair icon buttons
  with accessible labels or tooltips.
- Do not use decorative gradient blobs, orbs, or bokeh backgrounds.

## Typography

- Match text scale to context. Reserve hero-scale type for true hero sections
  and use smaller, tighter headings inside dashboards, panels, cards, and tool
  surfaces.
- Keep letter spacing at 0 unless a local brand rule requires otherwise.
- Do not scale font size directly with viewport width.

## Interaction

- Use segmented controls for modes, toggles or checkboxes for binary settings,
  sliders or steppers for numeric values, menus for option sets, and tabs for
  alternate views.
- Keep common workflows reachable without leaving the primary surface.
- Treat empty, loading, error, disabled, and selected states as part of the core
  UI, not as afterthoughts.

## Verification

- Check desktop and mobile viewports before handoff.
- Confirm text does not overflow or overlap.
- For frontend changes, include proof that the result follows this file or name
  any intentional deviation in the handoff.
