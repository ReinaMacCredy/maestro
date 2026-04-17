# External Libraries (CDN)

Optional CDN libraries for cases where pure CSS/HTML isn't enough. Only include what the blueprint actually needs -- most blueprints need Mermaid and Google Fonts, nothing else.

## Mermaid.js -- Diagramming Engine

Blueprints typically use Mermaid for architecture diagrams, dependency DAGs, and optionally sequence or ER diagrams.

**CDN:**
```html
<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';

  mermaid.initialize({ startOnLoad: true, /* ... */ });
</script>
```

**With ELK layout** (for complex graphs with 15+ nodes):
```html
<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
  import elkLayouts from 'https://cdn.jsdelivr.net/npm/@mermaid-js/layout-elk/dist/mermaid-layout-elk.esm.min.mjs';

  mermaid.registerLayoutLoaders(elkLayouts);
  mermaid.initialize({ startOnLoad: true, layout: 'elk', /* ... */ });
</script>
```

### Deep Theming

Always use `theme: 'base'` -- it's the only theme where all `themeVariables` are fully customizable.

```html
<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';

  const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  mermaid.initialize({
    startOnLoad: true,
    theme: 'base',
    look: 'classic',
    themeVariables: {
      primaryColor: isDark ? '#1e2538' : '#eff6ff',
      primaryBorderColor: isDark ? '#60a5fa' : '#2563eb',
      primaryTextColor: isDark ? '#e6edf3' : '#1a1a2e',
      secondaryColor: isDark ? '#1c2333' : '#f0fdf4',
      secondaryBorderColor: isDark ? '#34d399' : '#059669',
      secondaryTextColor: isDark ? '#e6edf3' : '#1a1a2e',
      tertiaryColor: isDark ? '#27201a' : '#fef3c7',
      tertiaryBorderColor: isDark ? '#fbbf24' : '#d97706',
      tertiaryTextColor: isDark ? '#e6edf3' : '#1a1a2e',
      lineColor: isDark ? '#6b7280' : '#9ca3af',
      fontSize: '16px',
      fontFamily: 'var(--font-body)',
      noteBkgColor: isDark ? '#1c2333' : '#fefce8',
      noteTextColor: isDark ? '#e6edf3' : '#1a1a2e',
      noteBorderColor: isDark ? '#fbbf24' : '#d97706',
    }
  });
</script>
```

### CSS Overrides on Mermaid SVG

```css
/* Force text colors to follow page theme */
.mermaid .nodeLabel { color: var(--text) !important; }
.mermaid .edgeLabel { color: var(--text-dim) !important; background-color: var(--bg) !important; }
.mermaid .edgeLabel rect { fill: var(--bg) !important; }

.mermaid .node rect,
.mermaid .node circle,
.mermaid .node polygon { stroke-width: 1.5px; }

.mermaid .edge-pattern-solid { stroke-width: 1.5px; }

.mermaid .edgeLabel {
  font-family: var(--font-mono) !important;
  font-size: 13px !important;
}

.mermaid .nodeLabel {
  font-family: var(--font-body) !important;
  font-size: 16px !important;
}
```

### classDef for Blueprint Diagrams

Use semi-transparent fills so they work in both light and dark themes.

```
%% Critical path (red/rose)
classDef critical fill:#dc262622,stroke:#dc2626,stroke-width:2px

%% New components (green)
classDef new fill:#16a34a22,stroke:#16a34a,stroke-width:2px

%% Modified components (amber)
classDef modified fill:#d9770622,stroke:#d97706,stroke-width:1.5px

%% Existing/unchanged (muted)
classDef existing fill:#6b728011,stroke:#6b728044,stroke-width:1px
```

### stateDiagram-v2 Label Limitations

Avoid colons, parentheses, `<br/>`, and special characters in state diagram labels. If you need them, use `flowchart` instead with quoted edge labels.

### Dark Mode

Mermaid initializes once and can't reactively switch themes. Read preference at load time:
```javascript
const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
```

### Zoom Controls JavaScript

Add once at the end of the page, after Mermaid init:

```javascript
function updateZoomState(wrap) {
  var target = wrap.querySelector('.mermaid');
  var zoom = parseFloat(target.dataset.zoom || '1');
  wrap.classList.toggle('is-zoomed', zoom > 1);
}

function zoomDiagram(btn, factor) {
  var wrap = btn.closest('.mermaid-wrap');
  var target = wrap.querySelector('.mermaid');
  var current = parseFloat(target.dataset.zoom || '1');
  var next = Math.min(Math.max(current * factor, 0.3), 5);
  target.dataset.zoom = next;
  target.style.transform = 'scale(' + next + ')';
  updateZoomState(wrap);
}

function resetZoom(btn) {
  var wrap = btn.closest('.mermaid-wrap');
  var target = wrap.querySelector('.mermaid');
  target.dataset.zoom = '1';
  target.style.transform = 'scale(1)';
  updateZoomState(wrap);
}

document.querySelectorAll('.mermaid-wrap').forEach(function(wrap) {
  wrap.addEventListener('wheel', function(e) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    var target = wrap.querySelector('.mermaid');
    var current = parseFloat(target.dataset.zoom || '1');
    var factor = e.deltaY < 0 ? 1.1 : 0.9;
    var next = Math.min(Math.max(current * factor, 0.3), 5);
    target.dataset.zoom = next;
    target.style.transform = 'scale(' + next + ')';
    updateZoomState(wrap);
  }, { passive: false });

  var startX, startY, scrollL, scrollT;
  wrap.addEventListener('mousedown', function(e) {
    if (e.target.closest('.zoom-controls')) return;
    var target = wrap.querySelector('.mermaid');
    if (parseFloat(target.dataset.zoom || '1') <= 1) return;
    wrap.classList.add('is-panning');
    startX = e.clientX;
    startY = e.clientY;
    scrollL = wrap.scrollLeft;
    scrollT = wrap.scrollTop;
  });
  window.addEventListener('mousemove', function(e) {
    if (!wrap.classList.contains('is-panning')) return;
    wrap.scrollLeft = scrollL - (e.clientX - startX);
    wrap.scrollTop = scrollT - (e.clientY - startY);
  });
  window.addEventListener('mouseup', function() {
    wrap.classList.remove('is-panning');
  });
});
```

## Google Fonts -- Typography

Always load with `display=swap`. Pick a distinctive pairing -- never default to Inter, Roboto, Arial, or system-ui.

```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Space+Mono:wght@400;700&family=Outfit:wght@400;500;600;700&display=swap" rel="stylesheet">
```

**Font suggestions** (rotate -- never use the same pairing twice in a row):

| Body / Headings | Mono / Labels | Feel |
|---|---|---|
| Outfit | Space Mono | Clean geometric, modern |
| Instrument Serif | JetBrains Mono | Editorial, refined |
| Sora | IBM Plex Mono | Technical, precise |
| DM Sans | Fira Code | Friendly, developer |
| Fraunces | Source Code Pro | Warm, distinctive |
| Manrope | Martian Mono | Soft, contemporary |
| Bricolage Grotesque | Fragment Mono | Bold, characterful |
| Geist | Geist Mono | Vercel-inspired, sharp |
| Plus Jakarta Sans | Azeret Mono | Rounded, approachable |
