---
name: maestro:content-engine
description: "Multi-platform content creation for X, LinkedIn, TikTok, YouTube, and newsletters. Use in any maestro phase when producing social media content or marketing materials."
lifecycle: [cross-cutting]
domain: [content:marketing]
---

# Content Engine

Turn one idea into strong, platform-native content instead of posting the same thing everywhere.

## Maestro Integration

**Lifecycle**: cross-cutting
**Activates when**: maestro:new-track detects relevant tech in tech-stack.md, or maestro:implement encounters matching task types.

### Phase Guidance
**In maestro:cross-cutting**: Apply content patterns when creating platform-native content. Adapt tone, format, and length per platform requirements.

### Related Skills
- maestro:article-writing
- maestro:investor-materials
- maestro:frontend-slides

## First Questions

Clarify:
- source asset: what are we adapting from
- audience: builders, investors, customers, operators, or general audience
- platform: X, LinkedIn, TikTok, YouTube, newsletter, or multi-platform
- goal: awareness, conversion, recruiting, authority, launch support, or engagement

## Core Rules

1. Adapt for the platform. Do not cross-post the same copy.
2. Hooks matter more than summaries.
3. Every post should carry one clear idea.
4. Use specifics over slogans.
5. Keep the ask small and clear.

## Platform Guidance

### X
- open fast
- one idea per post or per tweet in a thread
- keep links out of the main body unless necessary
- avoid hashtag spam

### LinkedIn
- strong first line
- short paragraphs
- more explicit framing around lessons, results, and takeaways

### TikTok / Short Video
- first 3 seconds must interrupt attention
- script around visuals, not just narration
- one demo, one claim, one CTA

### YouTube
- show the result early
- structure by chapter
- refresh the visual every 20-30 seconds

### Newsletter
- deliver one clear lens, not a bundle of unrelated items
- make section titles skimmable
- keep the opening paragraph doing real work

## Repurposing Flow

Default cascade:
1. anchor asset: article, video, demo, memo, or launch doc
2. extract 3-7 atomic ideas
3. write platform-native variants
4. trim repetition across outputs
5. align CTAs with platform intent

## Deliverables

When asked for a campaign, return:
- the core angle
- platform-specific drafts
- optional posting order
- optional CTA variants
- any missing inputs needed before publishing

## Quality Gate

Before delivering:
- each draft reads natively for its platform
- hooks are strong and specific
- no generic hype language
- no duplicated copy across platforms unless requested
- the CTA matches the content and audience

---
## Relationship to Maestro Workflow
- `/maestro:new-track` -- Detects this skill during Step 9.5 (skill matching)
- `/maestro:implement` -- Loads this skill's guidance during task execution
- `/maestro:review` -- Uses checklists as review criteria
