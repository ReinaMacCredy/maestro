# Cleanup Loose Types

I'll help improve type safety in your code by finding and fixing loose type declarations.

First, let me analyze your project to understand what needs attention. I'll look for:

- Loose or generic type declarations
- Missing type annotations where your language supports them
- Overly permissive types that could be more specific
- Type assertions that might hide issues
- Functions without proper type signatures

I'll scan your codebase based on file extensions and patterns I find. For each issue discovered, I'll:

1. Show you the current code with context
2. Analyze how the value is actually used
3. Suggest a more specific type based on usage patterns
4. Explain why the suggested type is better
5. Apply the change after your confirmation

My approach prioritizes:
- **Safety**: Never break existing functionality
- **Clarity**: Make types self-documenting
- **Maintainability**: Use types that prevent future bugs
- **Project conventions**: Follow your existing type patterns

This helps catch bugs at compile-time rather than runtime, making your code more robust and easier to maintain.