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
              primaryColor: '#ffffff',
              primaryTextColor: '#000000',
              primaryBorderColor: '#000000',
              secondaryColor: '#ffffff',
              secondaryTextColor: '#000000',
              secondaryBorderColor: '#000000',
              tertiaryColor: '#ffffff',
              tertiaryTextColor: '#000000',
              tertiaryBorderColor: '#000000',
              lineColor: '#000000',
              textColor: '#000000',
            },
            themeCSS: `
              .node rect, .node circle, .node ellipse, .node polygon, .node path {
                fill: #ffffff !important;
                stroke: #000000 !important;
              }
              .nodeLabel, .label text, .cluster-label text {
                color: #000000 !important;
                fill: #000000 !important;
              }
              .edgePath .path, .flowchart-link {
                stroke: #000000 !important;
              }
              .arrowheadPath, marker path {
                fill: #000000 !important;
                stroke: #000000 !important;
              }
              .edgeLabel {
                background-color: #ffffff !important;
                color: #000000 !important;
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
            { label: 'SLP setup and storage', slug: 'getting-started/slp-setup' },
            { label: 'Quick start', slug: 'getting-started/quick-start' },
          ],
        },
        {
          label: 'Concepts',
          items: [
            { label: 'Three layers', slug: 'concepts/three-layers' },
            { label: 'Roles', slug: 'concepts/roles' },
            { label: 'Team collaboration', slug: 'concepts/lanes' },
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
            { label: 'Supervised teams', slug: 'guides/supervised-teams' },
            { label: 'Self-improvement', slug: 'guides/self-improvement' },
            { label: 'Attention and brief', slug: 'guides/attention-and-brief' },
            { label: 'Read-only mode', slug: 'guides/observer-mode' },
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
