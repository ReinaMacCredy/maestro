import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://maestro.maccredyreina.me',
  base: '/',
  integrations: [
    starlight({
      title: 'maestro',
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
