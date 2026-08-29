import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import rehypeMermaid from 'rehype-mermaid';

export default defineConfig({
  site: 'https://maestro.maccredyreina.me',
  base: '/',
  markdown: {
    rehypePlugins: [
      [
        rehypeMermaid,
        {
          strategy: 'inline-svg',
          mermaidConfig: {
            theme: 'base',
            themeVariables: {
              background: '#ffffff',
              primaryColor: '#334155',
              primaryTextColor: '#ffffff',
              primaryBorderColor: '#64748b',
              secondaryColor: '#475569',
              secondaryTextColor: '#ffffff',
              secondaryBorderColor: '#64748b',
              tertiaryColor: '#e2e8f0',
              tertiaryTextColor: '#0f172a',
              tertiaryBorderColor: '#64748b',
              lineColor: '#64748b',
              textColor: '#334155',
            },
            themeCSS: `
              .node rect, .node circle, .node ellipse, .node polygon, .node path {
                fill: var(--sl-color-bg-accent) !important;
                stroke: var(--sl-color-accent) !important;
              }
              .nodeLabel, .label text, .cluster-label text {
                color: var(--sl-color-text) !important;
                fill: var(--sl-color-text) !important;
              }
              .edgePath .path, .flowchart-link {
                stroke: var(--sl-color-gray-3) !important;
              }
              .arrowheadPath, marker path {
                fill: var(--sl-color-gray-3) !important;
                stroke: var(--sl-color-gray-3) !important;
              }
              .edgeLabel {
                background-color: var(--sl-color-bg) !important;
                color: var(--sl-color-text) !important;
              }
            `,
          },
        },
      ],
    ],
  },
  integrations: [
    starlight({
      title: 'maestro',
      customCss: ['./src/styles/custom.css'],
      editLink: {
        baseUrl: 'https://github.com/ReinaMacCredy/maestro/edit/main/site/',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/ReinaMacCredy/maestro',
        },
      ],
      sidebar: [
        {
          label: 'Getting started',
          items: [
            { label: 'Overview', slug: 'index' },
            { label: 'Install', slug: 'getting-started/install' },
            { label: 'Quick start', slug: 'getting-started/quick-start' },
          ],
        },
        {
          label: 'Concepts',
          items: [
            { label: 'Three layers', slug: 'concepts/three-layers' },
            { label: 'Roles', slug: 'concepts/roles' },
            { label: 'Lanes', slug: 'concepts/lanes' },
            {
              label: 'Work, decisions, and evidence',
              slug: 'concepts/work-decisions-evidence',
            },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'SLP scenarios', slug: 'guides/slp-scenarios' },
            { label: 'Self-improvement', slug: 'guides/self-improvement' },
            { label: 'Attention and brief', slug: 'guides/attention-and-brief' },
            { label: 'Observer mode', slug: 'guides/observer-mode' },
            { label: 'Harness integration', slug: 'guides/harness-integration' },
            {
              label: 'Recipes, skills, and plugins',
              slug: 'guides/recipes-skills-plugins',
            },
            { label: 'Import Rust data', slug: 'guides/import-rust-data' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'CLI', slug: 'reference/cli' },
            { label: 'Configuration', slug: 'reference/configuration' },
          ],
        },
        { label: 'Troubleshooting', slug: 'troubleshooting' },
      ],
    }),
  ],
});
