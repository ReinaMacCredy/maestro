# CSS Patterns for Blueprints

Reusable patterns for blueprint-specific layouts: KPI summary, timeline, phase cards, file change indicators, risk matrix, dependency graphs, and verification checklists. Extends the general visual patterns with blueprint-specific components.

## Theme Setup

Always define both light and dark palettes. Blueprints benefit from semantic color naming tied to their domain.

```css
:root {
  --font-body: 'Outfit', system-ui, sans-serif;
  --font-mono: 'Space Mono', 'SF Mono', Consolas, monospace;

  --bg: #f8f9fa;
  --surface: #ffffff;
  --surface2: #f0f2f5;
  --surface-elevated: #ffffff;
  --border: rgba(0, 0, 0, 0.08);
  --border-bright: rgba(0, 0, 0, 0.15);
  --text: #1a1a2e;
  --text-dim: #6b7280;

  /* Blueprint semantic colors */
  --phase-active: #2563eb;
  --phase-active-dim: rgba(37, 99, 235, 0.08);
  --phase-done: #059669;
  --phase-done-dim: rgba(5, 150, 105, 0.08);
  --phase-future: #8b5cf6;
  --phase-future-dim: rgba(139, 92, 246, 0.08);

  --file-add: #16a34a;
  --file-add-dim: rgba(22, 163, 74, 0.08);
  --file-modify: #d97706;
  --file-modify-dim: rgba(217, 119, 6, 0.08);
  --file-delete: #dc2626;
  --file-delete-dim: rgba(220, 38, 38, 0.08);

  --risk-low: #059669;
  --risk-low-dim: rgba(5, 150, 105, 0.08);
  --risk-medium: #d97706;
  --risk-medium-dim: rgba(217, 119, 6, 0.08);
  --risk-high: #dc2626;
  --risk-high-dim: rgba(220, 38, 38, 0.08);

  --accent: #2563eb;
  --accent-dim: rgba(37, 99, 235, 0.08);
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0d1117;
    --surface: #161b22;
    --surface2: #1c2333;
    --surface-elevated: #21283b;
    --border: rgba(255, 255, 255, 0.06);
    --border-bright: rgba(255, 255, 255, 0.12);
    --text: #e6edf3;
    --text-dim: #8b949e;

    --phase-active: #60a5fa;
    --phase-active-dim: rgba(96, 165, 250, 0.12);
    --phase-done: #34d399;
    --phase-done-dim: rgba(52, 211, 153, 0.12);
    --phase-future: #a78bfa;
    --phase-future-dim: rgba(167, 139, 250, 0.12);

    --file-add: #4ade80;
    --file-add-dim: rgba(74, 222, 128, 0.12);
    --file-modify: #fbbf24;
    --file-modify-dim: rgba(251, 191, 36, 0.12);
    --file-delete: #f87171;
    --file-delete-dim: rgba(248, 113, 113, 0.12);

    --risk-low: #34d399;
    --risk-low-dim: rgba(52, 211, 153, 0.12);
    --risk-medium: #fbbf24;
    --risk-medium-dim: rgba(251, 191, 36, 0.12);
    --risk-high: #f87171;
    --risk-high-dim: rgba(248, 113, 113, 0.12);

    --accent: #60a5fa;
    --accent-dim: rgba(96, 165, 250, 0.12);
  }
}
```

## Background Atmosphere

```css
/* Blueprint grid feel */
body {
  background: var(--bg);
  background-image:
    radial-gradient(ellipse at 30% 0%, var(--accent-dim) 0%, transparent 50%),
    radial-gradient(circle, var(--border) 1px, transparent 1px);
  background-size: 100% 100%, 20px 20px;
}
```

## KPI Summary Cards

The executive summary uses large metric cards. Place in a responsive grid at the top of the blueprint.

```css
.kpi-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 16px;
  margin-bottom: 32px;
}

.kpi-card {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 20px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  animation: fadeScale 0.35s ease-out both;
  animation-delay: calc(var(--i, 0) * 0.06s);
}

.kpi-card__value {
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -1px;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}

.kpi-card__label {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 1.5px;
  color: var(--text-dim);
  margin-top: 6px;
}

/* Color variants */
.kpi-card--scope .kpi-card__value { color: var(--accent); }
.kpi-card--risk-low .kpi-card__value { color: var(--risk-low); }
.kpi-card--risk-medium .kpi-card__value { color: var(--risk-medium); }
.kpi-card--risk-high .kpi-card__value { color: var(--risk-high); }
```

## Problem Statement

Hero card for the problem context. Appears below KPIs.

```css
.problem-statement {
  background: color-mix(in srgb, var(--surface) 92%, var(--accent) 8%);
  border: 1px solid color-mix(in srgb, var(--border) 50%, var(--accent) 50%);
  border-radius: 12px;
  padding: 24px 28px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.06);
  font-size: 15px;
  line-height: 1.7;
  margin-bottom: 32px;
}

.problem-statement strong { color: var(--text); }
```

## Timeline / Phased Implementation

Vertical timeline with phase markers and content cards branching to one side.

```css
.timeline {
  position: relative;
  padding-left: 40px;
  margin: 24px 0;
}

/* Central vertical line */
.timeline::before {
  content: '';
  position: absolute;
  left: 15px;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--border-bright);
}

.timeline-phase {
  position: relative;
  margin-bottom: 32px;
  animation: fadeUp 0.4s ease-out both;
  animation-delay: calc(var(--i, 0) * 0.08s);
}

.timeline-phase:last-child { margin-bottom: 0; }

/* Phase marker dot on the line */
.timeline-phase::before {
  content: '';
  position: absolute;
  left: -33px;
  top: 6px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid var(--phase-active);
  background: var(--bg);
  z-index: 1;
}

.timeline-phase--done::before {
  background: var(--phase-done);
  border-color: var(--phase-done);
}

.timeline-phase--future::before {
  border-color: var(--phase-future);
  opacity: 0.6;
}

.timeline-phase__header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.timeline-phase__num {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 1.5px;
  color: var(--phase-active);
}

.timeline-phase__title {
  font-size: 18px;
  font-weight: 600;
}

.timeline-phase__duration {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-dim);
  margin-left: auto;
  padding: 3px 8px;
  background: var(--surface2);
  border-radius: 4px;
}

.timeline-phase__body {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 20px;
}
```

## Phase Detail Cards (Collapsible)

Per-phase expandable sections containing file changes, tasks, and test plans.

```css
details.phase-detail {
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: hidden;
  margin-bottom: 12px;
}

details.phase-detail summary {
  padding: 14px 20px;
  background: var(--surface);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  list-style: none;
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text);
  transition: background 0.15s ease;
}

details.phase-detail summary:hover {
  background: var(--surface-elevated);
}

details.phase-detail summary::-webkit-details-marker { display: none; }

details.phase-detail summary::before {
  content: '\25B8';
  font-size: 11px;
  color: var(--text-dim);
  transition: transform 0.15s ease;
}

details.phase-detail[open] summary::before {
  transform: rotate(90deg);
}

details.phase-detail .phase-content {
  padding: 16px 20px;
  border-top: 1px solid var(--border);
}
```

## File Change Indicators

Show which files are added, modified, or deleted.

```css
.file-list {
  list-style: none;
  padding: 0;
  margin: 0;
  font-size: 13px;
  line-height: 1.8;
}

.file-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 0;
  border-bottom: 1px solid var(--border);
}

.file-item:last-child { border-bottom: none; }

.file-badge {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 4px;
  min-width: 32px;
  text-align: center;
  flex-shrink: 0;
}

.file-badge--add { background: var(--file-add-dim); color: var(--file-add); }
.file-badge--mod { background: var(--file-modify-dim); color: var(--file-modify); }
.file-badge--del { background: var(--file-delete-dim); color: var(--file-delete); }

.file-path {
  font-family: var(--font-mono);
  font-size: 12px;
}

.file-reason {
  color: var(--text-dim);
  font-size: 12px;
  margin-left: auto;
}
```

Usage:
```html
<ul class="file-list">
  <li class="file-item">
    <span class="file-badge file-badge--add">+</span>
    <span class="file-path">src/ws/handler.ts</span>
    <span class="file-reason">WebSocket connection handler</span>
  </li>
  <li class="file-item">
    <span class="file-badge file-badge--mod">~</span>
    <span class="file-path">src/server.ts</span>
    <span class="file-reason">Add WS upgrade middleware</span>
  </li>
  <li class="file-item">
    <span class="file-badge file-badge--del">-</span>
    <span class="file-path">src/polling.ts</span>
    <span class="file-reason">Replaced by WebSocket</span>
  </li>
</ul>
```

## Task Cards (within phases)

Individual task items within a phase.

```css
.task-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-card {
  background: var(--surface2);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 12px 16px;
  display: flex;
  align-items: flex-start;
  gap: 12px;
  transition: border-color 0.15s;
}

.task-card:hover { border-color: var(--border-bright); }

.task-num {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  min-width: 24px;
  height: 24px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: var(--accent-dim);
  color: var(--accent);
}

.task-content { min-width: 0; flex: 1; }

.task-name {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 4px;
}

.task-desc {
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.5;
}

.task-tags {
  display: flex;
  gap: 6px;
  margin-top: 8px;
  flex-wrap: wrap;
}

.task-tag {
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 500;
  padding: 2px 6px;
  border-radius: 3px;
  background: var(--surface);
  border: 1px solid var(--border);
  color: var(--text-dim);
}
```

## Risk Matrix Table

Impact vs likelihood with colored severity indicators.

```css
.risk-table-wrap {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.risk-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.risk-table th {
  background: var(--surface-elevated);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: var(--text-dim);
  text-align: left;
  padding: 12px 16px;
  border-bottom: 2px solid var(--border-bright);
  white-space: nowrap;
}

.risk-table td {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  vertical-align: top;
}

.risk-table tbody tr:nth-child(even) {
  background: var(--surface2);
}

.risk-table tbody tr:last-child td {
  border-bottom: none;
}

.risk-level {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  padding: 3px 8px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.risk-level::before {
  content: '';
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

.risk-level--low { background: var(--risk-low-dim); color: var(--risk-low); }
.risk-level--medium { background: var(--risk-medium-dim); color: var(--risk-medium); }
.risk-level--high { background: var(--risk-high-dim); color: var(--risk-high); }
```

Usage:
```html
<div class="risk-table-wrap">
  <table class="risk-table">
    <thead>
      <tr><th>Risk</th><th>Impact</th><th>Likelihood</th><th>Mitigation</th></tr>
    </thead>
    <tbody>
      <tr>
        <td>DB migration failure</td>
        <td><span class="risk-level risk-level--high">High</span></td>
        <td><span class="risk-level risk-level--low">Low</span></td>
        <td>Reversible migration scripts, tested on staging first</td>
      </tr>
    </tbody>
  </table>
</div>
```

## Verification Checklist

Interactive-looking (but static) checklist for testing and acceptance criteria.

```css
.checklist {
  list-style: none;
  padding: 0;
  margin: 0;
}

.check-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.check-item:last-child { border-bottom: none; }

.check-box {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border-bright);
  border-radius: 4px;
  flex-shrink: 0;
  margin-top: 2px;
}

.check-item__text {
  font-size: 13px;
  line-height: 1.5;
}

.check-item__cmd {
  display: block;
  font-family: var(--font-mono);
  font-size: 12px;
  background: var(--surface2);
  padding: 4px 8px;
  border-radius: 4px;
  margin-top: 4px;
  color: var(--accent);
}

.check-item__expected {
  display: block;
  font-size: 11px;
  color: var(--text-dim);
  margin-top: 2px;
}
```

Usage:
```html
<ul class="checklist">
  <li class="check-item">
    <div class="check-box"></div>
    <div class="check-item__text">
      Build compiles without errors
      <code class="check-item__cmd">bun run build</code>
      <span class="check-item__expected">Exit code 0, no type errors</span>
    </div>
  </li>
</ul>
```

## Section Headings

Blueprint sections use numbered headings with colored dot indicators.

```css
.bp-section-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 40px 0 20px;
  scroll-margin-top: 24px;
}

.bp-section-num {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  min-width: 28px;
  height: 28px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--accent-dim);
  color: var(--accent);
  flex-shrink: 0;
}

.bp-section-title {
  font-size: 22px;
  font-weight: 700;
  letter-spacing: -0.5px;
}
```

## Mermaid Zoom Controls

Every Mermaid diagram in the blueprint needs zoom controls. Copy the full pattern from the visual-explainer CSS patterns (`.mermaid-wrap`, `.zoom-controls`, and the zoom/pan JavaScript). The patterns are identical -- read `../references/libraries.md` for the Mermaid init config.

```css
.mermaid-wrap {
  position: relative;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 32px 24px;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
  margin: 16px 0;
}

.mermaid-wrap .mermaid {
  transition: transform 0.2s ease;
  transform-origin: top center;
}

.zoom-controls {
  position: absolute;
  top: 8px;
  right: 8px;
  display: flex;
  gap: 2px;
  z-index: 10;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 2px;
}

.zoom-controls button {
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--text-dim);
  font-family: var(--font-mono);
  font-size: 14px;
  cursor: pointer;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s;
}

.zoom-controls button:hover {
  background: var(--border);
  color: var(--text);
}

.mermaid-wrap.is-zoomed { cursor: grab; }
.mermaid-wrap.is-panning { cursor: grabbing; user-select: none; }
```

## Overflow Protection

```css
.grid > *, .flex > *,
[style*="display: grid"] > *,
[style*="display: flex"] > * {
  min-width: 0;
}

body { overflow-wrap: break-word; }
```

## Animations

```css
@keyframes fadeUp {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes fadeScale {
  from { opacity: 0; transform: scale(0.92); }
  to { opacity: 1; transform: scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-delay: 0ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

## Responsive

```css
@media (max-width: 768px) {
  body { padding: 20px; }
  .kpi-row { grid-template-columns: 1fr 1fr; }
  .timeline { padding-left: 32px; }
  .timeline::before { left: 11px; }
  .timeline-phase::before { left: -29px; }
}

@media (max-width: 480px) {
  .kpi-row { grid-template-columns: 1fr; }
}
```
