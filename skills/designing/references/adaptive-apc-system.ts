/**
 * Adaptive A/P/C System - Unified State Machine
 * Combines: #4 Nudge, #3 Upgrade Path, #5 Branch-aware DS
 */

// =============================================================================
// TYPES
// =============================================================================

type DesignSupportState =
  | 'INLINE'
  | 'MICRO_APC'
  | 'NUDGE'
  | 'DS_FULL'
  | 'DS_BRANCH'
  | 'BRANCH_MERGE';

type DesignMode = 'SPEED' | 'FULL';
type DSPhase = 'DISCOVER' | 'DEFINE' | 'DEVELOP' | 'VERIFY';
type Checkpoint = 'CP1' | 'CP2' | 'CP3' | 'CP4';
type APCChoice = 'A' | 'P' | 'C' | 'BACK';
type MergeStrategy = 'overwrite' | 'new_track' | 'document_only';

type EventType =
  // Explicit commands
  | 'CMD_DS'                    // User types `ds` or `/conductor-design`
  | 'CMD_DS_BRANCH'             // User explicitly requests design branch
  // Passive triggers
  | 'CHECKPOINT_BOUNDARY'       // End of spec/plan/change detected
  | 'ITERATION_THRESHOLD'       // 3+ design iterations on same topic
  | 'DESIGN_RETHINK_DETECTED'   // "flow feels wrong" in implementation
  // User responses
  | 'USER_CHOICE_A'             // Advanced
  | 'USER_CHOICE_P'             // Party
  | 'USER_CHOICE_C'             // Continue
  | 'USER_CHOICE_BACK'          // Go back
  | 'USER_ACCEPT'               // Accept nudge/upgrade
  | 'USER_DECLINE'              // Decline nudge/upgrade
  | 'USER_EXIT_DS'              // Exit DS early
  // DS lifecycle
  | 'DS_PHASE_COMPLETE'         // Phase finished, show A/P/C
  | 'DS_SESSION_COMPLETE'       // All phases done
  | 'BRANCH_READY_TO_MERGE'     // Branch design complete
  // Merge options
  | 'MERGE_OVERWRITE'           // M1: Replace current design
  | 'MERGE_NEW_TRACK'           // M2: Create new track
  | 'MERGE_DOCUMENT_ONLY'       // M3: Keep as alternative
  | 'MERGE_CANCEL';             // Cancel merge

interface Event {
  type: EventType;
  payload?: {
    topicId?: string;
    trackId?: string;
    artifactType?: 'spec' | 'plan' | 'design_md' | 'code';
    complexityScore?: number;
    phaseHint?: DSPhase;
    summary?: string;
  };
}

interface DSState {
  mode: DesignMode;
  phase: DSPhase;
  checkpoint?: Checkpoint;
  lastApcChoice?: APCChoice;
  iterationsInPhase: number;
  complexityScore: number;
  seedContext?: string;  // Pre-filled context from conversation
}

interface BranchState {
  status: 'none' | 'proposed' | 'active' | 'merge_pending';
  branchId?: string;
  parentTrackId?: string;
  scopeSummary?: string;
  createdAtStep?: number;
  mergeStrategy?: MergeStrategy;
}

interface ConversationContext {
  // Current global state
  designSupportState: DesignSupportState;
  currentStep: number;
  
  // Track context
  activeTrackId?: string;
  activeTopicId?: string;
  currentSkill: 'conductor' | 'beads' | 'design' | 'inline';
  
  // Iteration tracking (for nudge #4)
  topicIterations: Record<string, number>;
  
  // Cooldowns
  lastNudgeStepByTopic: Record<string, number>;
  lastMicroStepByTopic: Record<string, number>;
  nudgeCooldownSteps: number;   // Default: 10
  microCooldownSteps: number;   // Default: 3
  
  // DS substate
  ds: DSState;
  
  // Branch substate (#5)
  branch: BranchState;
  
  // Checkpoint context
  checkpoint?: {
    kind: 'micro' | 'nudge' | 'ds_cp';
    artifactType?: string;
    artifactId?: string;
    suggestedUpgrade?: 'none' | 'offer_ds' | 'offer_branch';
  };
  
  // User preferences
  preferences: {
    defaultDesignMode: DesignMode;
    nudgeSensitivity: 'low' | 'normal' | 'high';
    suppressNudges: boolean;
    suppressMicro: boolean;
  };
}

interface TransitionResult {
  newState: DesignSupportState;
  prompt?: string;
  actions?: Action[];
}

type Action =
  | { type: 'START_DS'; mode: DesignMode; phase: DSPhase; seedContext?: string }
  | { type: 'START_BRANCH'; trackId: string; scopeSummary: string }
  | { type: 'SHOW_MICRO_APC'; options: string[] }
  | { type: 'SHOW_NUDGE'; message: string }
  | { type: 'ADVANCE_DS_PHASE'; nextPhase: DSPhase }
  | { type: 'COMPLETE_DS'; designDoc: string }
  | { type: 'EXECUTE_MERGE'; strategy: MergeStrategy }
  | { type: 'SET_COOLDOWN'; kind: 'micro' | 'nudge'; topicId: string }
  | { type: 'INCREMENT_ITERATIONS'; topicId: string }
  | { type: 'HANDOFF'; command: 'cn' | 'ci' | 'fb' };

// =============================================================================
// GUARDS (Condition Checks)
// =============================================================================

function isInCooldown(
  ctx: ConversationContext,
  kind: 'micro' | 'nudge',
  topicId: string
): boolean {
  const lastStep = kind === 'micro'
    ? ctx.lastMicroStepByTopic[topicId] ?? 0
    : ctx.lastNudgeStepByTopic[topicId] ?? 0;
  const cooldown = kind === 'micro'
    ? ctx.microCooldownSteps
    : ctx.nudgeCooldownSteps;
  return (ctx.currentStep - lastStep) < cooldown;
}

function hasEnoughIterations(ctx: ConversationContext, topicId: string): boolean {
  const threshold = ctx.preferences.nudgeSensitivity === 'high' ? 2
    : ctx.preferences.nudgeSensitivity === 'low' ? 4
    : 3;
  return (ctx.topicIterations[topicId] ?? 0) >= threshold;
}

function isInActiveDS(ctx: ConversationContext): boolean {
  return ctx.designSupportState === 'DS_FULL' || ctx.designSupportState === 'DS_BRANCH';
}

function hasActiveTrack(ctx: ConversationContext): boolean {
  return !!ctx.activeTrackId;
}

function shouldPreferBranch(ctx: ConversationContext): boolean {
  // In implementation mode with existing design that would be invalidated
  return hasActiveTrack(ctx) && ctx.currentSkill === 'conductor';
}

function determineDesignMode(complexityScore: number, preference: DesignMode): DesignMode {
  // SPEED for quick iterations, FULL for complex multi-phase
  if (complexityScore >= 7) return 'FULL';
  if (complexityScore <= 3) return 'SPEED';
  return preference;
}

function determineStartingPhase(ctx: ConversationContext, event: Event): DSPhase {
  const hint = event.payload?.phaseHint;
  if (hint) return hint;
  
  // Heuristics based on artifact type
  const artifact = event.payload?.artifactType;
  if (artifact === 'spec') return 'DEFINE';
  if (artifact === 'plan') return 'DEVELOP';
  if (artifact === 'code') return 'DEVELOP';
  return 'DISCOVER';
}

// =============================================================================
// STATE MACHINE DISPATCHER
// =============================================================================

function stepDesignSupport(ctx: ConversationContext, event: Event): TransitionResult {
  const state = ctx.designSupportState;
  const topicId = event.payload?.topicId ?? ctx.activeTopicId ?? 'default';
  
  // Priority 1: Explicit commands always win
  if (event.type === 'CMD_DS') {
    return handleExplicitDS(ctx, event, topicId);
  }
  
  if (event.type === 'CMD_DS_BRANCH') {
    return handleExplicitBranch(ctx, event, topicId);
  }
  
  // Priority 2: Block passive triggers if already in DS
  if (isInActiveDS(ctx) && isPassiveTrigger(event.type)) {
    return { newState: state }; // No-op, stay in current DS
  }
  
  // Dispatch based on current state
  switch (state) {
    case 'INLINE':
      return handleInlineState(ctx, event, topicId);
    case 'MICRO_APC':
      return handleMicroAPCState(ctx, event, topicId);
    case 'NUDGE':
      return handleNudgeState(ctx, event, topicId);
    case 'DS_FULL':
      return handleDSFullState(ctx, event, topicId);
    case 'DS_BRANCH':
      return handleDSBranchState(ctx, event, topicId);
    case 'BRANCH_MERGE':
      return handleBranchMergeState(ctx, event, topicId);
    default:
      return { newState: 'INLINE' };
  }
}

function isPassiveTrigger(eventType: EventType): boolean {
  return [
    'CHECKPOINT_BOUNDARY',
    'ITERATION_THRESHOLD',
    'DESIGN_RETHINK_DETECTED'
  ].includes(eventType);
}

// =============================================================================
// STATE HANDLERS
// =============================================================================

function handleExplicitDS(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  const complexity = event.payload?.complexityScore ?? 5;
  const mode = determineDesignMode(complexity, ctx.preferences.defaultDesignMode);
  const phase = determineStartingPhase(ctx, event);
  
  return {
    newState: 'DS_FULL',
    actions: [
      { type: 'START_DS', mode, phase, seedContext: event.payload?.summary }
    ],
    prompt: PROMPTS.DS_START(mode, phase)
  };
}

function handleExplicitBranch(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  if (!hasActiveTrack(ctx)) {
    return {
      newState: 'INLINE',
      prompt: PROMPTS.NO_TRACK_FOR_BRANCH
    };
  }
  
  return {
    newState: 'DS_BRANCH',
    actions: [
      { type: 'START_BRANCH', trackId: ctx.activeTrackId!, scopeSummary: event.payload?.summary ?? '' },
      { type: 'START_DS', mode: 'FULL', phase: 'DEVELOP' }
    ],
    prompt: PROMPTS.BRANCH_START(ctx.activeTrackId!, event.payload?.summary)
  };
}

function handleInlineState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  switch (event.type) {
    case 'CHECKPOINT_BOUNDARY':
      // Priority 4: Micro A/P/C
      if (!ctx.preferences.suppressMicro && !isInCooldown(ctx, 'micro', topicId)) {
        const suggestBranch = shouldPreferBranch(ctx);
        return {
          newState: 'MICRO_APC',
          actions: [
            { type: 'SHOW_MICRO_APC', options: suggestBranch ? ['A_BRANCH', 'P', 'C'] : ['A', 'P', 'C'] }
          ],
          prompt: suggestBranch ? PROMPTS.MICRO_APC_WITH_BRANCH : PROMPTS.MICRO_APC_STANDARD
        };
      }
      return { newState: 'INLINE' };
      
    case 'ITERATION_THRESHOLD':
      // Priority 5: Nudge (3+ iterations)
      if (!ctx.preferences.suppressNudges && 
          hasEnoughIterations(ctx, topicId) && 
          !isInCooldown(ctx, 'nudge', topicId)) {
        return {
          newState: 'NUDGE',
          actions: [{ type: 'SHOW_NUDGE', message: PROMPTS.NUDGE_MESSAGE }],
          prompt: PROMPTS.NUDGE_MESSAGE
        };
      }
      return { newState: 'INLINE' };
      
    case 'DESIGN_RETHINK_DETECTED':
      // Priority 3: Branch safety (design rethink in implementation)
      if (hasActiveTrack(ctx)) {
        return {
          newState: 'MICRO_APC',
          actions: [
            { type: 'SHOW_MICRO_APC', options: ['A_BRANCH', 'P', 'C'] }
          ],
          prompt: PROMPTS.DESIGN_RETHINK_DETECTED
        };
      }
      return { newState: 'INLINE' };
      
    default:
      return { newState: 'INLINE' };
  }
}

function handleMicroAPCState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  switch (event.type) {
    case 'USER_CHOICE_C':
      return {
        newState: 'INLINE',
        actions: [{ type: 'SET_COOLDOWN', kind: 'micro', topicId }],
        prompt: PROMPTS.CONTINUE_INLINE
      };
      
    case 'USER_CHOICE_A':
      // Escalate to FULL DS
      return {
        newState: 'DS_FULL',
        actions: [
          { type: 'START_DS', mode: 'FULL', phase: determineStartingPhase(ctx, event) }
        ],
        prompt: PROMPTS.UPGRADE_TO_DS('Advanced')
      };
      
    case 'USER_CHOICE_P':
      // Escalate to FULL DS with Party mode
      return {
        newState: 'DS_FULL',
        actions: [
          { type: 'START_DS', mode: 'FULL', phase: determineStartingPhase(ctx, event) }
        ],
        prompt: PROMPTS.UPGRADE_TO_DS('Party')
      };
      
    case 'USER_DECLINE':
      return {
        newState: 'INLINE',
        actions: [{ type: 'SET_COOLDOWN', kind: 'micro', topicId }],
        prompt: PROMPTS.MICRO_DECLINED
      };
      
    // Special: A with branch option
    case 'CMD_DS_BRANCH':
      if (hasActiveTrack(ctx)) {
        return {
          newState: 'DS_BRANCH',
          actions: [
            { type: 'START_BRANCH', trackId: ctx.activeTrackId!, scopeSummary: event.payload?.summary ?? '' },
            { type: 'START_DS', mode: 'FULL', phase: 'DEVELOP' }
          ],
          prompt: PROMPTS.BRANCH_START(ctx.activeTrackId!, event.payload?.summary)
        };
      }
      return { newState: 'DS_FULL', actions: [{ type: 'START_DS', mode: 'FULL', phase: 'DEVELOP' }] };
      
    default:
      return { newState: 'MICRO_APC' };
  }
}

function handleNudgeState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  switch (event.type) {
    case 'USER_ACCEPT':
      const complexity = event.payload?.complexityScore ?? 5;
      const mode = determineDesignMode(complexity, ctx.preferences.defaultDesignMode);
      return {
        newState: 'DS_FULL',
        actions: [
          { type: 'START_DS', mode, phase: 'DISCOVER', seedContext: event.payload?.summary }
        ],
        prompt: PROMPTS.NUDGE_ACCEPTED(mode)
      };
      
    case 'USER_DECLINE':
      return {
        newState: 'INLINE',
        actions: [{ type: 'SET_COOLDOWN', kind: 'nudge', topicId }],
        prompt: PROMPTS.NUDGE_DECLINED
      };
      
    default:
      return { newState: 'NUDGE' };
  }
}

function handleDSFullState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  switch (event.type) {
    case 'DS_PHASE_COMPLETE':
      // Show A/P/C checkpoint within DS
      return {
        newState: 'DS_FULL',
        prompt: PROMPTS.DS_CHECKPOINT(ctx.ds.phase, ctx.ds.checkpoint!)
      };
      
    case 'USER_CHOICE_A':
      return {
        newState: 'DS_FULL',
        prompt: PROMPTS.DS_ADVANCED(ctx.ds.phase)
      };
      
    case 'USER_CHOICE_P':
      return {
        newState: 'DS_FULL',
        prompt: PROMPTS.DS_PARTY(ctx.ds.phase)
      };
      
    case 'USER_CHOICE_C':
      const nextPhase = getNextPhase(ctx.ds.phase);
      if (nextPhase) {
        return {
          newState: 'DS_FULL',
          actions: [{ type: 'ADVANCE_DS_PHASE', nextPhase }],
          prompt: PROMPTS.DS_NEXT_PHASE(nextPhase)
        };
      }
      // DS complete
      return {
        newState: 'INLINE',
        actions: [
          { type: 'COMPLETE_DS', designDoc: 'design.md' },
          { type: 'HANDOFF', command: 'ci' }
        ],
        prompt: PROMPTS.DS_COMPLETE
      };
      
    case 'USER_CHOICE_BACK':
      const prevPhase = getPrevPhase(ctx.ds.phase);
      if (prevPhase) {
        return {
          newState: 'DS_FULL',
          actions: [{ type: 'ADVANCE_DS_PHASE', nextPhase: prevPhase }],
          prompt: PROMPTS.DS_BACK_PHASE(prevPhase)
        };
      }
      return { newState: 'DS_FULL' };
      
    case 'CMD_DS_BRANCH':
      // Fork to branch from within DS
      if (hasActiveTrack(ctx)) {
        return {
          newState: 'DS_BRANCH',
          actions: [
            { type: 'START_BRANCH', trackId: ctx.activeTrackId!, scopeSummary: event.payload?.summary ?? '' }
          ],
          prompt: PROMPTS.DS_TO_BRANCH
        };
      }
      return { newState: 'DS_FULL' };
      
    case 'USER_EXIT_DS':
      return {
        newState: 'INLINE',
        prompt: PROMPTS.DS_EXIT_EARLY
      };
      
    case 'DS_SESSION_COMPLETE':
      return {
        newState: 'INLINE',
        actions: [
          { type: 'COMPLETE_DS', designDoc: 'design.md' },
          { type: 'HANDOFF', command: 'ci' }
        ],
        prompt: PROMPTS.DS_COMPLETE
      };
      
    default:
      return { newState: 'DS_FULL' };
  }
}

function handleDSBranchState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  // Similar to DS_FULL but with branch awareness
  switch (event.type) {
    case 'DS_SESSION_COMPLETE':
    case 'BRANCH_READY_TO_MERGE':
      return {
        newState: 'BRANCH_MERGE',
        prompt: PROMPTS.BRANCH_MERGE_OPTIONS(ctx.branch.scopeSummary ?? '')
      };
      
    case 'USER_EXIT_DS':
      return {
        newState: 'INLINE',
        prompt: PROMPTS.BRANCH_ABANDONED
      };
      
    // A/P/C within branch DS - same as DS_FULL
    case 'USER_CHOICE_A':
    case 'USER_CHOICE_P':
    case 'USER_CHOICE_C':
    case 'USER_CHOICE_BACK':
      return handleDSFullState(ctx, event, topicId);
      
    default:
      return { newState: 'DS_BRANCH' };
  }
}

function handleBranchMergeState(ctx: ConversationContext, event: Event, topicId: string): TransitionResult {
  switch (event.type) {
    case 'MERGE_OVERWRITE':
      return {
        newState: 'INLINE',
        actions: [{ type: 'EXECUTE_MERGE', strategy: 'overwrite' }],
        prompt: PROMPTS.MERGE_COMPLETE('overwrite')
      };
      
    case 'MERGE_NEW_TRACK':
      return {
        newState: 'INLINE',
        actions: [
          { type: 'EXECUTE_MERGE', strategy: 'new_track' },
          { type: 'HANDOFF', command: 'cn' }
        ],
        prompt: PROMPTS.MERGE_COMPLETE('new_track')
      };
      
    case 'MERGE_DOCUMENT_ONLY':
      return {
        newState: 'INLINE',
        actions: [{ type: 'EXECUTE_MERGE', strategy: 'document_only' }],
        prompt: PROMPTS.MERGE_COMPLETE('document_only')
      };
      
    case 'MERGE_CANCEL':
      return {
        newState: 'INLINE',
        prompt: PROMPTS.MERGE_CANCELLED
      };
      
    default:
      return { newState: 'BRANCH_MERGE' };
  }
}

// =============================================================================
// HELPERS
// =============================================================================

function getNextPhase(current: DSPhase): DSPhase | null {
  const order: DSPhase[] = ['DISCOVER', 'DEFINE', 'DEVELOP', 'VERIFY'];
  const idx = order.indexOf(current);
  return idx < order.length - 1 ? order[idx + 1] : null;
}

function getPrevPhase(current: DSPhase): DSPhase | null {
  const order: DSPhase[] = ['DISCOVER', 'DEFINE', 'DEVELOP', 'VERIFY'];
  const idx = order.indexOf(current);
  return idx > 0 ? order[idx - 1] : null;
}

// =============================================================================
// PROMPT TEMPLATES
// =============================================================================

const PROMPTS = {
  // Micro A/P/C
  MICRO_APC_STANDARD: `
Design checkpoint:
- **[A]** Advanced – deeper design exploration
- **[P]** Party – multi-perspective feedback
- **[C]** Continue inline
`,

  MICRO_APC_WITH_BRANCH: `
Design checkpoint (changes diverge from current design):
- **[A]** Advanced – explore alternatives in a design branch
- **[P]** Party – get multi-perspective feedback
- **[C]** Continue as-is
`,

  DESIGN_RETHINK_DETECTED: `
You're signaling the current design doesn't feel right.
- **[A]** Start a design branch – explore alternatives safely
- **[P]** Get opinions first
- **[C]** Keep current plan
`,

  // Nudge
  NUDGE_MESSAGE: `
We've iterated on this flow several times. Want to switch into a structured **Design Session** with A/P/C checkpoints to properly explore options?

- **[Start Design Session]** _(recommended)_
- **[Not now]**
`,

  NUDGE_ACCEPTED: (mode: DesignMode) => `
Starting ${mode} Design Session. I'll import our recent discussion as context and begin at DISCOVER phase.
`,

  NUDGE_DECLINED: `Ok, continuing inline. I won't suggest this again for a while.`,

  // DS Lifecycle
  DS_START: (mode: DesignMode, phase: DSPhase) => `
Starting ${mode} Design Session at ${phase} phase.
`,

  DS_CHECKPOINT: (phase: DSPhase, cp: Checkpoint) => `
**${cp} Checkpoint** (${phase} phase complete)

- **[A]** Advanced – ${getAdvancedDescription(phase)}
- **[P]** Party – multi-agent design review
- **[C]** Continue to next phase
- **[↩ Back]** – return to previous phase
`,

  DS_ADVANCED: (phase: DSPhase) => `Running Advanced check for ${phase}...`,
  DS_PARTY: (phase: DSPhase) => `Starting Party Mode review for ${phase}...`,
  DS_NEXT_PHASE: (phase: DSPhase) => `Moving to ${phase} phase...`,
  DS_BACK_PHASE: (phase: DSPhase) => `Returning to ${phase} phase...`,
  DS_COMPLETE: `
Design Session complete. Design document ready.

Next steps:
- \`cn\` – Create new track from this design
- \`ci\` – Implement in current track
- \`fb\` – File beads from design
`,

  DS_EXIT_EARLY: `Exiting Design Session. Partial progress saved.`,

  UPGRADE_TO_DS: (mode: string) => `
Upgrading to FULL Design Session with ${mode} analysis. Importing current context...
`,

  // Branch
  BRANCH_START: (trackId: string, scope?: string) => `
Created design branch for Track \`${trackId}\`.
${scope ? `Scope: ${scope}` : ''}

Running FULL Double Diamond with A/P/C. Original track untouched until merge.
`,

  DS_TO_BRANCH: `Forking current DS into a design branch...`,

  BRANCH_MERGE_OPTIONS: (scope: string) => `
**Design branch complete**: ${scope}

How to apply this design?
- **[M1]** Replace current design/plan for this track
- **[M2]** Create new implementation track
- **[M3]** Keep as documented alternative (no changes yet)
- **[Cancel]** Discard branch
`,

  MERGE_COMPLETE: (strategy: MergeStrategy) => {
    switch (strategy) {
      case 'overwrite': return 'Design merged. Spec/plan updated. Affected beads tagged for review.';
      case 'new_track': return 'New track created from branch design. Original track unchanged.';
      case 'document_only': return 'Branch saved as alternative design. No implementation changes.';
    }
  },

  MERGE_CANCELLED: `Branch discarded. Returning to original track.`,
  BRANCH_ABANDONED: `Design branch abandoned. Original track unchanged.`,

  // Misc
  CONTINUE_INLINE: `Continuing inline.`,
  MICRO_DECLINED: `Ok, continuing.`,
  NO_TRACK_FOR_BRANCH: `No active track to branch from. Use \`ds\` for a standalone Design Session.`,
};

function getAdvancedDescription(phase: DSPhase): string {
  switch (phase) {
    case 'DISCOVER': return 'challenge assumptions, explore edge cases';
    case 'DEFINE': return 'stress-test problem definition';
    case 'DEVELOP': return 'deep dive on solution alternatives';
    case 'VERIFY': return 'Oracle audit before finalizing';
  }
}

// =============================================================================
// INTEGRATION HOOKS
// =============================================================================

/**
 * Call this on every user turn to detect passive triggers
 */
function detectPassiveTriggers(ctx: ConversationContext, userMessage: string): Event[] {
  const events: Event[] = [];
  const topicId = ctx.activeTopicId ?? 'default';
  
  // Detect design rethink phrases
  const rethinkPatterns = [
    /flow.*(wrong|off|bad)/i,
    /rethink.*(ux|design|flow)/i,
    /doesn't (make sense|feel right)/i,
    /designed.*(wrong|incorrectly)/i,
    // Vietnamese
    /flow.*(sai|lỗi)/i,
    /thiết kế.*(lại|sai)/i,
    /không.*(hợp lý|đúng)/i,
  ];
  
  if (rethinkPatterns.some(p => p.test(userMessage))) {
    events.push({ type: 'DESIGN_RETHINK_DETECTED', payload: { topicId } });
  }
  
  // Detect design iteration (increment counter)
  const iterationPatterns = [
    /try (another|different)/i,
    /what if/i,
    /iterate/i,
    /rework/i,
    /still not (sure|right)/i,
    // Vietnamese
    /thử (cách khác|lại)/i,
    /nếu như/i,
    /chưa (ổn|đúng)/i,
  ];
  
  if (iterationPatterns.some(p => p.test(userMessage))) {
    ctx.topicIterations[topicId] = (ctx.topicIterations[topicId] ?? 0) + 1;
    if (hasEnoughIterations(ctx, topicId)) {
      events.push({ type: 'ITERATION_THRESHOLD', payload: { topicId } });
    }
  }
  
  return events;
}

/**
 * Call this after generating spec/plan/design content
 */
function detectCheckpointBoundary(ctx: ConversationContext, artifactType: string): Event | null {
  // Only in INLINE mode
  if (ctx.designSupportState !== 'INLINE') return null;
  
  const topicId = ctx.activeTopicId ?? 'default';
  if (isInCooldown(ctx, 'micro', topicId)) return null;
  
  return {
    type: 'CHECKPOINT_BOUNDARY',
    payload: { topicId, artifactType: artifactType as any }
  };
}

// =============================================================================
// EXPORTS
// =============================================================================

export {
  ConversationContext,
  Event,
  EventType,
  TransitionResult,
  stepDesignSupport,
  detectPassiveTriggers,
  detectCheckpointBoundary,
  PROMPTS,
};
