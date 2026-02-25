# HTML & CSS Style Guide

## HTML

### Structure
- Use semantic elements: `<header>`, `<nav>`, `<main>`, `<section>`, `<article>`, `<footer>`
- One `<main>` per page
- Use `<button>` for actions, `<a>` for navigation
- Use `<form>` for data submission
- Avoid `<div>` soup -- use semantic elements first

### Attributes
- Use double quotes for attribute values
- Order: `id`, `class`, `data-*`, `src`/`href`, `type`, `aria-*`, `role`
- Use `lang` attribute on `<html>`
- Use `alt` on all `<img>` elements (empty `alt=""` for decorative images)

### Accessibility
- Every form input needs a `<label>` (or `aria-label`)
- Use `aria-live` for dynamic content updates
- Ensure keyboard navigation works (tab order, focus indicators)
- Color contrast ratio: 4.5:1 for normal text, 3:1 for large text
- Touch targets: minimum 44x44px
- Skip navigation link for keyboard users
- Use `role` attributes for custom interactive elements

### Performance
- Load critical CSS inline in `<head>`
- Defer non-critical JS with `defer` attribute
- Use `loading="lazy"` for below-fold images
- Minimize DOM depth (avoid excessive nesting)

## CSS

### Methodology
- BEM naming: `.block__element--modifier`
- Or utility-first (Tailwind): use design tokens consistently
- Component-scoped styles preferred (CSS Modules, scoped, or shadow DOM)

### Naming
- Classes: kebab-case (`.my-component`)
- BEM: `.card`, `.card__title`, `.card--featured`
- State classes: `.is-active`, `.is-hidden`, `.has-error`
- JS hooks: `[data-*]` attributes (not classes)

### Properties
- Order: positioning, box model, typography, visual, misc
  ```css
  .element {
    /* Positioning */
    position: relative;
    top: 0;
    z-index: 1;
    /* Box model */
    display: flex;
    width: 100%;
    padding: 1rem;
    margin: 0;
    /* Typography */
    font-size: 1rem;
    line-height: 1.5;
    color: var(--text-primary);
    /* Visual */
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 0.25rem;
    /* Misc */
    transition: opacity 0.2s ease;
    cursor: pointer;
  }
  ```

### Modern CSS
- Use CSS custom properties (`--var`) for theming
- Use `clamp()` for responsive sizing
- Use CSS Grid for 2D layouts, Flexbox for 1D
- Use `gap` instead of margin hacks for spacing
- Use `aspect-ratio` for media containers
- Use `@container` queries for component-level responsiveness
- Use `prefers-color-scheme` for dark mode
- Use `prefers-reduced-motion` for accessibility

### Responsive Design
- Mobile-first: base styles for small screens, `@media` for larger
- Use relative units: `rem`, `em`, `%`, `vw/vh`
- Breakpoints: use project-defined tokens, not magic numbers
- Test at standard breakpoints: 320px, 768px, 1024px, 1440px

### Anti-patterns
- No `!important` (except for utility overrides)
- No `id` selectors for styling
- No inline styles (except dynamic values via JS)
- No pixel values for font-size (use rem)
- No magic numbers -- extract to custom properties
- No vendor prefixes manually -- use Autoprefixer
