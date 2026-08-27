# Maestro documentation site

The Starlight site renders `mermaid` fences during `astro build` with
`rehype-mermaid` and the `inline-svg` strategy. The generated HTML contains the
SVG, so the deployed site does not load Mermaid or a CDN at runtime. Diagram
styles use Starlight color variables and follow its light and dark themes.

Builds need Playwright's Chromium binary in addition to the package dependency:

```sh
bun install --frozen-lockfile
bun x playwright install --with-deps chromium
bun run build
```

CI runners must install Chromium before `bun run build`.
