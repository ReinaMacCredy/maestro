---
name: playwright
description: Browser automation for E2E testing and visual QA. Use for testing UI, screenshots, form interactions, page navigation.
allowed-tools: Bash, Read, Write
model: sonnet
---

# Playwright Browser Automation

Browser automation agent for E2E testing and visual verification.

## Prerequisites

Playwright requires browser installation:

```bash
npx playwright install chromium
```

Or with all dependencies:

```bash
npx playwright install --with-deps chromium
```

## Capabilities

- **Page Navigation**: Navigate to URLs, wait for page loads
- **Element Interaction**: Click, type, select, hover
- **Form Handling**: Fill forms, submit, handle validation
- **Screenshots**: Capture full page or element screenshots
- **Assertions**: Wait for elements, check visibility, verify text

## Common Operations

### Navigate and Screenshot

```typescript
import { chromium } from 'playwright';

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto('https://example.com');
await page.screenshot({ path: 'screenshot.png', fullPage: true });
await browser.close();
```

### Form Interaction

```typescript
await page.fill('input[name="email"]', 'user@example.com');
await page.fill('input[name="password"]', 'secret');
await page.click('button[type="submit"]');
await page.waitForNavigation();
```

### Wait for Elements

```typescript
await page.waitForSelector('.success-message');
await page.waitForLoadState('networkidle');
await page.waitForTimeout(1000); // Last resort
```

### Element Assertions

```typescript
await expect(page.locator('.title')).toHaveText('Welcome');
await expect(page.locator('button')).toBeEnabled();
await expect(page.locator('.error')).not.toBeVisible();
```

## MCP Integration

When using Playwright MCP server:

```json
{
  "playwright": {
    "command": "npx",
    "args": ["-y", "@anthropic-ai/mcp-playwright"]
  }
}
```

The MCP server provides direct browser control tools:
- `playwright_navigate`: Go to a URL
- `playwright_screenshot`: Capture the page
- `playwright_click`: Click an element
- `playwright_fill`: Type into an input
- `playwright_evaluate`: Run JavaScript

## Testing Patterns

### Visual Regression

```typescript
const screenshot = await page.screenshot();
await expect(screenshot).toMatchSnapshot('homepage.png');
```

### Responsive Testing

```typescript
const viewports = [
  { width: 375, height: 667, name: 'mobile' },
  { width: 768, height: 1024, name: 'tablet' },
  { width: 1440, height: 900, name: 'desktop' }
];

for (const vp of viewports) {
  await page.setViewportSize({ width: vp.width, height: vp.height });
  await page.screenshot({ path: `${vp.name}.png` });
}
```

### Authentication Flow

```typescript
// Save auth state for reuse
await page.context().storageState({ path: 'auth.json' });

// Reuse in other tests
const context = await browser.newContext({ storageState: 'auth.json' });
```

## Best Practices

1. **Use stable selectors**: Prefer `data-testid` over classes
2. **Wait properly**: Use `waitForSelector` not arbitrary timeouts
3. **Handle flakiness**: Retry failed assertions
4. **Clean up**: Always close browsers in finally blocks
5. **Parallel safe**: Each test should be independent

## Invocation Examples

```
@playwright take a screenshot of the homepage
@playwright test the login form with invalid credentials
@playwright check if the dashboard loads correctly
@playwright capture screenshots at all viewports
```
