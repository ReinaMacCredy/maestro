import { describe, expect, it } from "bun:test";
import { createInitialState, reduce, type AppState, type Action } from "@/tui/state/reducer.js";
import type { MissionControlSnapshot } from "@/tui/state/types.js";

function makeSnapshot(overrides?: Partial<MissionControlSnapshot>): MissionControlSnapshot {
  return {
      mode: "mission",
      missionId: "2026-03-30-001",
    missionTitle: "Test",
    missionStatus: "executing",
    effectiveStatus: "executing",
    elapsedMs: 0,
    featureProgress: { done: 0, total: 3, active: 0 },
    statusProgress: {
      completed: 0,
      total: 3,
      inFlight: 0,
      blocked: 0,
      queued: 3,
      completionPct: 0,
    },
    tokenCounters: null,
    activeFeature: {
      id: "f1",
      title: "Feature 1",
      status: "pending",
      milestoneId: "m1",
      milestoneTitle: "Milestone 1",
      workerType: "test",
      description: "Test",
      preconditions: undefined,
      expectedBehavior: undefined,
      verificationSteps: [],
      dependsOn: [],
      fulfills: [],
      validTransitions: ["assigned", "in-progress"],
      },
      features: [
        { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
        { id: "f2", title: "F2", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
        { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "test", hasReport: false },
      ],
      session: {
        branch: "main",
        workingTreeClean: false,
        diffStat: "+4 -1",
        changedFiles: ["src/tui/index.ts"],
      },
      pendingHandoffs: [],
      configSummary: {
        configSource: "project",
        cassAvailable: true,
        gitAvailable: true,
        checks: [],
        missionDirectory: ".maestro/missions/2026-03-30-001",
        workerTypes: ["test"],
        backgroundMode: "solid",
      },
      progressLog: [],
      milestones: [],
      canPause: true,
      canResume: false,
      home: null,
      ...overrides,
    };
}

function makeState(overrides?: Partial<AppState>): AppState {
  return {
    ...createInitialState(makeSnapshot()),
    ...overrides,
  };
}

  describe("createInitialState", () => {
    it("sets default focus to features", () => {
      const state = createInitialState(makeSnapshot());
      expect(state.focusedPanel).toBe("features");
      expect(state.selectedFeatureIndex).toBe(0);
      expect(state.modal.kind).toBe("none");
      expect(state.running).toBe(true);
    });

    // Phase 3 strip: the "follows the live feature immediately" test
    // relied on runtimeProcesses / activeWorker to auto-select a live
    // feature index. Both stores were deleted so auto-follow is gone.
  });

describe("reduce", () => {
  describe("quit", () => {
    it("sets running to false", () => {
      const state = reduce(makeState(), { type: "quit" });
      expect(state.running).toBe(false);
    });
  });

  describe("navigate", () => {
    it("moves feature selection down", () => {
      const state = reduce(makeState(), { type: "navigate", direction: "down" });
      expect(state.selectedFeatureIndex).toBe(1);
    });

    it("moves feature selection up", () => {
      const s1 = reduce(makeState(), { type: "navigate", direction: "down" });
      const s2 = reduce(s1, { type: "navigate", direction: "up" });
      expect(s2.selectedFeatureIndex).toBe(0);
    });

    it("clamps at bounds", () => {
      const state = reduce(makeState(), { type: "navigate", direction: "up" });
      expect(state.selectedFeatureIndex).toBe(0);
    });

    it("navigates modal options when modal is open", () => {
      const state = makeState({
        modal: { kind: "feature-action", featureIndex: 0, selectedOption: 0, phase: "selecting" },
      });
      const next = reduce(state, { type: "navigate", direction: "down" });
      if (next.modal.kind === "feature-action") {
        expect(next.modal.selectedOption).toBe(1);
        expect(next.modal.phase).toBe("selecting");
      }
    });

    it("navigates the feature browser instead of the background feature list", () => {
      const state = makeState({
        selectedFeatureIndex: 0,
        modal: { kind: "feature-browser", selectedFeatureIndex: 0 },
      });
      const next = reduce(state, { type: "navigate", direction: "down" });

      expect(next.selectedFeatureIndex).toBe(0);
      expect(next.modal.kind).toBe("feature-browser");
      if (next.modal.kind === "feature-browser") {
        expect(next.modal.selectedFeatureIndex).toBe(1);
      }
    });

      it("returns palette-backed overlays to the command palette on left-arrow back", () => {
        const state = makeState({
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
            returnTarget: "command-palette",
            returnPalette: { query: "hand", selectedCommandIndex: 0 },
          },
        });
        const next = reduce(state, { type: "navigate", direction: "left" });

        expect(next.modal.kind).toBe("command-palette");
        if (next.modal.kind === "command-palette") {
          expect(next.modal.query).toBe("hand");
          expect(next.modal.selectedCommandIndex).toBe(0);
        }
      });

      it("preserves the active command palette query when returning from a palette-launched overlay", () => {
        const paletteState = reduce(
          reduce(
            reduce(makeState(), { type: "open-command-palette" }),
            { type: "modal-query-append", char: "h" },
          ),
          { type: "modal-query-append", char: "a" },
        );
        const overlayState = reduce(paletteState, { type: "open-handoffs" });
        const next = reduce(overlayState, { type: "navigate", direction: "left" });

        expect(overlayState.modal.kind).toBe("handoffs");
        expect(next.modal.kind).toBe("command-palette");
        if (next.modal.kind === "command-palette") {
          expect(next.modal.query).toBe("ha");
          expect(next.modal.selectedCommandIndex).toBe(0);
          }
        });

      it("preserves the palette query after navigating inside palette-launched handoffs", () => {
        const paletteState = reduce(
          reduce(
            reduce(makeState({
                snapshot: makeSnapshot({
                  pendingHandoffs: [
                    { id: "h1", agent: "codex", message: "one", timestamp: "2026-04-15T00:00:00.000Z" },
                    { id: "h2", agent: "claude-code", message: "two", timestamp: "2026-04-15T00:01:00.000Z" },
                  ],
                }),
            }), { type: "open-command-palette" }),
            { type: "modal-query-append", char: "h" },
          ),
          { type: "modal-query-append", char: "a" },
        );
        const overlayState = reduce(paletteState, { type: "open-handoffs" });
        const navigated = reduce(overlayState, { type: "navigate", direction: "down" });
        const next = reduce(navigated, { type: "navigate", direction: "left" });

        expect(next.modal.kind).toBe("command-palette");
        if (next.modal.kind === "command-palette") {
          expect(next.modal.query).toBe("ha");
          expect(next.modal.selectedCommandIndex).toBe(0);
        }
      });

      it("ignores left-arrow on the command palette home view", () => {
        const state = makeState({
        modal: { kind: "command-palette", query: "han", selectedCommandIndex: 1 },
      });
      const next = reduce(state, { type: "navigate", direction: "left" });

      expect(next).toEqual(state);
    });
  });

  describe("focus", () => {
    it("switches focused panel", () => {
      const state = reduce(makeState(), { type: "focus", panel: "log" });
      expect(state.focusedPanel).toBe("log");
    });

        it("does not change focus when modal is open", () => {
          const state = makeState({
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 0,
              phase: "browse",
              selectedScope: "project",
            },
          });
        const next = reduce(state, { type: "focus", panel: "log" });
        expect(next.focusedPanel).toBe("features");
    });
  });

    describe("enter", () => {
      it("opens feature action modal when features focused", () => {
        const state = reduce(makeState(), { type: "enter" });
        expect(state.modal.kind).toBe("feature-action");
        if (state.modal.kind === "feature-action") {
          expect(state.modal.phase).toBe("selecting");
        }
      });

      it("moves feature action modal into confirming state on enter", () => {
        const opened = reduce(makeState(), { type: "enter" });
        const confirmed = reduce(opened, { type: "enter" });
        expect(confirmed.modal.kind).toBe("feature-action");
        if (confirmed.modal.kind === "feature-action") {
          expect(confirmed.modal.phase).toBe("confirming");
        }
      });

      it("does nothing in home mode", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            mode: "home",
            home: {
              headline: "No project detected",
              summary: "Open a repo",
              locationLabel: "Outside a git repository",
              checks: [],
              actions: [],
              pendingHandoffs: [],
            },
          }),
        });
        const next = reduce(state, { type: "enter" });
        expect(next.modal.kind).toBe("none");
      });

      it("does nothing when no features", () => {
          const state = makeState({
          snapshot: makeSnapshot({ features: [] }),
        });
        const next = reduce(state, { type: "enter" });
        expect(next.modal.kind).toBe("none");
      });

      it("opens config editing directly from the effective tab", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            configInspector: {
              tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
              rowsByTab: {
                overview: [],
                effective: [{
                  keyPath: "execution.stopOnFailure",
                  label: "Stop on failure",
                  section: "Execution",
                  valueText: "on",
                  displayValueText: "on",
                  source: "default",
                  sourceBadge: "D",
                  editKind: "toggle",
                  editKindLabel: "on/off",
                  options: ["off", "on"],
                  description: "desc",
                  summary: "Choose whether Maestro stops after the first failed task.",
                  impactText: "If this is on, the run stops on the first failure.",
                  effectiveValueText: "on",
                  effectiveDisplayValueText: "on",
                }],
                project: [],
                global: [],
                defaults: [],
                workers: [],
                plan: [],
                doctor: [],
                memory: [],
              },
              hasProjectConfig: true,
              hasGlobalConfig: true,
              projectPath: ".maestro/config.yaml",
              globalPath: "~/.maestro/config.yaml",
              errors: [],
            },
          }),
          modal: {
            kind: "config",
            tab: "effective",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
          },
        });

        const next = reduce(state, { type: "enter" });

        expect(next.modal.kind).toBe("config");
        if (next.modal.kind === "config") {
          expect(next.modal.phase).toBe("edit-inline");
        }
      });
    });

    describe("escape", () => {
        it("closes config row finder before closing the config modal", () => {
          const state = makeState({
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 1,
              phase: "browse",
              selectedScope: "project",
              findQuery: "worker",
            },
          });
          const next = reduce(state, { type: "escape" });

          expect(next.modal.kind).toBe("config");
          if (next.modal.kind === "config") {
            expect(next.modal.findQuery).toBeUndefined();
            expect(next.modal.selectedRowIndex).toBe(0);
          }
        });

          it("closes modal", () => {
            const state = makeState({
              modal: {
                kind: "config",
                tab: "overview",
                selectedRowIndex: 0,
                phase: "browse",
                selectedScope: "project",
              },
            });
          const next = reduce(state, { type: "escape" });
          expect(next.modal.kind).toBe("none");
      });

        it("closes palette-launched detail modals instead of returning to the command palette", () => {
          const state = makeState({
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 0,
              phase: "browse",
              selectedScope: "project",
              returnTarget: "command-palette",
            },
          });
        const next = reduce(state, { type: "escape" });

        expect(next.modal.kind).toBe("none");
      });

      it("closes the command palette itself instead of reopening it", () => {
        const state = makeState({
          modal: { kind: "command-palette", query: "han", selectedCommandIndex: 2 },
        });
        const next = reduce(state, { type: "escape" });

        expect(next.modal.kind).toBe("none");
      });

      it("unfocuses panel when no modal", () => {
        const state = reduce(makeState(), { type: "escape" });
        expect(state.focusedPanel).toBe("none");
      });

        it("turns off copy mode before changing focus", () => {
          const state = makeState({ copyMode: true, focusedPanel: "features", leftPaneMode: "preview" });
          const next = reduce(state, { type: "escape" });

          expect(next.copyMode).toBe(false);
          expect(next.leftPaneMode).toBe("preview");
        });

        it("turns off copy mode while closing an open modal", () => {
          const state = makeState({
            copyMode: true,
            modal: { kind: "command-palette", query: "dep", selectedCommandIndex: 0 },
          });
          const next = reduce(state, { type: "escape" });

          expect(next.modal.kind).toBe("none");
          expect(next.copyMode).toBe(false);
        });
      });

    describe("open-features", () => {
    it("opens feature browser", () => {
      const state = reduce(makeState(), { type: "open-features" });
      expect(state.modal.kind).toBe("feature-browser");
    });

      it("opens the feature browser from the command palette", () => {
        const state = reduce(
          makeState({ modal: { kind: "command-palette", query: "fea", selectedCommandIndex: 1 } }),
          { type: "open-features" },
        );
        expect(state.modal.kind).toBe("feature-browser");
        if (state.modal.kind === "feature-browser") {
          expect(state.modal.returnTarget).toBe("command-palette");
        }
      });
    });

    describe("open-handoffs", () => {
      it("opens handoffs modal", () => {
        const state = reduce(makeState(), { type: "open-handoffs" });
        expect(state.modal.kind).toBe("handoffs");
      });

      it("keeps palette return context when opened from the command palette", () => {
        const state = reduce(
          makeState({ modal: { kind: "command-palette", query: "han", selectedCommandIndex: 0 } }),
          { type: "open-handoffs" },
        );

        expect(state.modal.kind).toBe("handoffs");
        if (state.modal.kind === "handoffs") {
          expect(state.modal.returnTarget).toBe("command-palette");
        }
        });

        it("keeps the split handoff overlay open on enter and closes on escape", () => {
            const opened = reduce(makeState({
              snapshot: makeSnapshot({
                pendingHandoffs: [
                  { id: "h1", message: "Review this", agent: "codex", timestamp: "2026-04-15T00:00:00.000Z" },
                ],
              }),
            }), { type: "open-handoffs" });
          const detail = reduce(opened, { type: "enter" });

          expect(detail.modal.kind).toBe("handoffs");

          const closed = reduce(detail, { type: "escape" });
          expect(closed.modal.kind).toBe("none");
        });
      });

      describe("open-config", () => {
        it("opens config modal", () => {
          const state = reduce(makeState(), { type: "open-config" });
          expect(state.modal.kind).toBe("config");
          if (state.modal.kind === "config") {
            expect(state.modal.findQuery).toBeUndefined();
          }
        });

        it("returns palette-backed config overlays to the command palette on left-arrow back", () => {
          const state = makeState({
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 0,
              phase: "browse",
              selectedScope: "project",
              returnTarget: "command-palette",
              returnPalette: { query: "conf", selectedCommandIndex: 0 },
            },
          });

          const next = reduce(state, { type: "navigate", direction: "left" });

          expect(next.modal.kind).toBe("command-palette");
          if (next.modal.kind === "command-palette") {
            expect(next.modal.query).toBe("conf");
            expect(next.modal.selectedCommandIndex).toBe(0);
          }
        });
      });

      // Phase 3 strip: open-processes, open-workers, and open-runtime-output
      // modals are deleted in Commit 3.2. Their tests referenced
      // runtimeProcesses/workerHealth/activeWorker fields that were
      // removed from MissionControlSnapshot in Commit 3.1.

      describe("open-dependencies", () => {
        it("opens dependencies modal", () => {
          const state = reduce(makeState(), { type: "open-dependencies" });
          expect(state.modal.kind).toBe("dependencies");
        });

        it("does not open dependencies from home mode", () => {
          const state = reduce(makeState({
            snapshot: makeSnapshot({
              mode: "home",
              home: {
                headline: "Home",
                summary: "No mission",
                locationLabel: "repo",
                checks: [],
                actions: [],
                pendingHandoffs: [],
              },
            }),
          }), { type: "open-dependencies" });

          expect(state.modal.kind).toBe("none");
        });

        it("jumps to a selected dependency on enter", () => {
        const snapshot = makeSnapshot({
          features: [
            { id: "f1", title: "F1", status: "done", milestoneId: "m1", workerType: "test", hasReport: true },
            { id: "f2", title: "F2", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
            { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "test", hasReport: false },
          ],
          taskPreviews: [
            {
              id: "f1",
              title: "F1",
              status: "done",
              milestoneId: "m1",
              milestoneTitle: "Milestone 1",
              workerType: "test",
              description: "Done",
              preconditions: undefined,
              expectedBehavior: undefined,
              verificationSteps: [],
              dependsOn: [],
              fulfills: [],
              validTransitions: [],
            },
            {
              id: "f2",
              title: "F2",
              status: "pending",
              milestoneId: "m1",
              milestoneTitle: "Milestone 1",
              workerType: "test",
              description: "Blocked work",
              preconditions: undefined,
              expectedBehavior: undefined,
              verificationSteps: [],
              dependsOn: ["f1"],
              blockedBy: [{ id: "f1", title: "F1", status: "done" }],
              unblocks: [{ id: "f3", title: "F3", status: "pending" }],
              fulfills: [],
              validTransitions: ["assigned"],
            },
            {
              id: "f3",
              title: "F3",
              status: "pending",
              milestoneId: "m2",
              milestoneTitle: "Milestone 2",
              workerType: "test",
              description: "Downstream work",
              preconditions: undefined,
              expectedBehavior: undefined,
              verificationSteps: [],
              dependsOn: ["f2"],
              fulfills: [],
              validTransitions: ["assigned"],
            },
          ],
        });
        const opened = reduce(makeState({
          snapshot,
          selectedFeatureIndex: 1,
        }), { type: "open-dependencies" });

        expect(opened.modal.kind).toBe("dependencies");

        const jumped = reduce(opened, { type: "enter" });
        expect(jumped.modal.kind).toBe("none");
        expect(jumped.selectedFeatureIndex).toBe(0);
        expect(jumped.leftPaneMode).toBe("preview");
      });
    });

      describe("command palette", () => {
    it("opens the command palette with a fresh query", () => {
      const state = reduce(makeState(), { type: "open-command-palette" });
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("appends query characters and resets selection", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "", selectedCommandIndex: 3 } }),
        { type: "modal-query-append", char: "p" },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("p");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("backspaces the query and resets selection", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "proc", selectedCommandIndex: 2 } }),
        { type: "modal-query-backspace" },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.query).toBe("pro");
        expect(state.modal.selectedCommandIndex).toBe(0);
      }
    });

    it("updates the selected command index", () => {
      const state = reduce(
        makeState({ modal: { kind: "command-palette", query: "", selectedCommandIndex: 0 } }),
        { type: "modal-select", option: 2 },
      );
      expect(state.modal.kind).toBe("command-palette");
      if (state.modal.kind === "command-palette") {
        expect(state.modal.selectedCommandIndex).toBe(2);
      }
    });

    it("does not open over another active modal", () => {
      const state = reduce(
        makeState({
          modal: { kind: "feature-action", featureIndex: 0, selectedOption: 0, phase: "confirming" },
        }),
        { type: "open-command-palette" },
      );

      expect(state.modal.kind).toBe("feature-action");
    });

      it("keeps keyboard navigation within the filtered results", () => {
        let state = makeState({
          modal: { kind: "command-palette", query: "proc", selectedCommandIndex: 0 },
        });

      state = reduce(state, { type: "navigate", direction: "down" });
      state = reduce(state, { type: "navigate", direction: "down" });

      expect(state.modal.kind).toBe("command-palette");
        if (state.modal.kind === "command-palette") {
          expect(state.modal.selectedCommandIndex).toBe(0);
        }
      });

      describe("config row finder", () => {
        it("starts a row finder query in browse mode", () => {
          const state = makeState({
            modal: {
              kind: "config",
              tab: "overview",
              selectedRowIndex: 1,
              phase: "browse",
              selectedScope: "project",
            },
          });
          const next = reduce(state, { type: "config-find-start" });

          expect(next.modal.kind).toBe("config");
          if (next.modal.kind === "config") {
            expect(next.modal.findQuery).toBe("");
            expect(next.modal.selectedRowIndex).toBe(0);
          }
        });
      });

      it("returns palette-launched feature browser overlays to the palette after snapshot refresh", () => {
        const state = makeState({
          modal: { kind: "feature-browser", selectedFeatureIndex: 1, returnTarget: "command-palette" },
        });
        const reordered = makeSnapshot({
          features: [
            { id: "f2", title: "F2", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
            { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "test", hasReport: false },
            { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "test", hasReport: false },
          ],
        });

        const next = reduce(state, { type: "update-snapshot", snapshot: reordered });

        expect(next.modal.kind).toBe("feature-browser");
        if (next.modal.kind === "feature-browser") {
          expect(next.modal.returnTarget).toBe("command-palette");
        }
      });
    });

    describe("update-snapshot", () => {
    it("updates snapshot and clamps selection", () => {
      const state = makeState({ selectedFeatureIndex: 5 });
      const newSnap = makeSnapshot({
        features: [
          { id: "f1", title: "F1", status: "done", milestoneId: "m1", workerType: "t", hasReport: false },
        ],
      });
      const next = reduce(state, { type: "update-snapshot", snapshot: newSnap });
      expect(next.snapshot.features.length).toBe(1);
      expect(next.selectedFeatureIndex).toBe(0);
    });

    it("preserves the selected feature by id when the snapshot order changes", () => {
      const state = makeState({ selectedFeatureIndex: 1 });
      const reordered = makeSnapshot({
        features: [
          { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "t", hasReport: false },
          { id: "f2", title: "F2", status: "assigned", milestoneId: "m1", workerType: "t", hasReport: false },
          { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "t", hasReport: false },
        ],
      });
      const next = reduce(state, { type: "update-snapshot", snapshot: reordered });
      expect(next.selectedFeatureIndex).toBe(1);
      expect(next.snapshot.features[next.selectedFeatureIndex]?.id).toBe("f2");
    });

      it("preserves the feature browser selection by id when the snapshot order changes", () => {
      const state = makeState({
        selectedFeatureIndex: 1,
        modal: { kind: "feature-browser", selectedFeatureIndex: 1 },
      });
      const reordered = makeSnapshot({
        features: [
          { id: "f2", title: "F2", status: "assigned", milestoneId: "m1", workerType: "t", hasReport: false },
          { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "t", hasReport: false },
          { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "t", hasReport: false },
        ],
      });

      const next = reduce(state, { type: "update-snapshot", snapshot: reordered });

      expect(next.modal.kind).toBe("feature-browser");
        if (next.modal.kind === "feature-browser") {
          expect(next.modal.selectedFeatureIndex).toBe(0);
          expect(next.snapshot.features[next.modal.selectedFeatureIndex]?.id).toBe("f2");
        }
      });

      // Phase 3 strip: auto-follow of live runtime features is gone.
      // It relied on runtimeProcesses + activeWorker to detect that a
      // previously pending feature moved into a running state. Without
      // those stores there is no live-feature signal to follow.

      it("does not mutate focus when a config modal is open and a snapshot lands", () => {
        const state = makeState({
          leftPaneMode: "overview",
          selectedFeatureIndex: 0,
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
          },
        });
        const nextSnapshot = makeSnapshot({
          features: [
            { id: "f1", title: "F1", status: "pending", milestoneId: "m1", workerType: "t", hasReport: false },
            { id: "f2", title: "F2", status: "in-progress", milestoneId: "m1", workerType: "t", hasReport: false },
            { id: "f3", title: "F3", status: "pending", milestoneId: "m2", workerType: "t", hasReport: false },
          ],
        });

        const next = reduce(state, { type: "update-snapshot", snapshot: nextSnapshot });

        expect(next.selectedFeatureIndex).toBe(0);
        expect(next.leftPaneMode).toBe("overview");
        expect(next.modal.kind).toBe("config");
      });

      // Phase 3 strip: the "preserves process selection by feature id
      // when runtime items reorder" test covered the runtimeProcesses
      // modal which is deleted in Commit 3.2.

      describe("config scope flow", () => {
      it("toggles the save scope inline when S is pressed", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            configInspector: {
              tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
              rowsByTab: {
                overview: [{
                  keyPath: "execution.stopOnFailure",
                  label: "Stop on failure",
                  section: "Quick settings",
                  valueText: "on",
                  displayValueText: "on",
                  source: "default",
                  sourceBadge: "D",
                  editKind: "toggle",
                  editKindLabel: "on/off",
                  options: ["off", "on"],
                  description: "desc",
                  summary: "Choose whether Maestro stops after the first failed task.",
                  impactText: "If this is on, the run stops on the first failure.",
                  effectiveValueText: "on",
                  effectiveDisplayValueText: "on",
                }],
                effective: [],
                project: [],
                global: [],
                defaults: [],
                workers: [],
                plan: [],
                doctor: [],
                memory: [],
              },
              hasProjectConfig: true,
              hasGlobalConfig: true,
              projectPath: ".maestro/config.yaml",
              globalPath: "~/.maestro/config.yaml",
              errors: [],
            },
          }),
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
          },
        });

        const next = reduce(state, { type: "config-toggle-scope" });

        expect(next.modal.kind).toBe("config");
        if (next.modal.kind === "config") {
          expect(next.modal.phase).toBe("browse");
          expect(next.modal.selectedScope).toBe("global");
        }
      });

      it("forces global scope for global-only settings", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            configInspector: {
              tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
              rowsByTab: {
                overview: [{
                  keyPath: "ui.missionControl.backgroundMode",
                  label: "Background mode",
                  section: "Quick settings",
                  valueText: "terminal",
                  displayValueText: "terminal background",
                  source: "global",
                  sourceBadge: "G",
                  editKind: "enum",
                  editKindLabel: "choice",
                  options: ["solid", "terminal"],
                  description: "Use a solid fill or let the terminal background show through.",
                  summary: "Choose whether Mission Control paints a solid backdrop.",
                  impactText: "Terminal background mode keeps normal chrome transparent.",
                  effectiveValueText: "terminal",
                  effectiveDisplayValueText: "terminal background",
                }],
                effective: [],
                project: [],
                global: [],
                defaults: [],
                workers: [],
                plan: [],
                doctor: [],
                memory: [],
              },
              hasProjectConfig: true,
              hasGlobalConfig: true,
              projectPath: ".maestro/config.yaml",
              globalPath: "~/.maestro/config.yaml",
              errors: [],
            },
          }),
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
          },
        });

        const next = reduce(state, { type: "config-toggle-scope" });

        expect(next.modal.kind).toBe("config");
        if (next.modal.kind === "config") {
          expect(next.modal.phase).toBe("browse");
          expect(next.modal.selectedScope).toBe("global");
        }
      });

      it("keeps inline editing active when the scope is toggled", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            configInspector: {
              tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
              rowsByTab: {
                overview: [{
                  keyPath: "execution.stopOnFailure",
                  label: "Stop on failure",
                  section: "Quick settings",
                  valueText: "on",
                  displayValueText: "on",
                  source: "default",
                  sourceBadge: "D",
                  editKind: "toggle",
                  editKindLabel: "on/off",
                  options: ["off", "on"],
                  description: "desc",
                  summary: "Choose whether Maestro stops after the first failed task.",
                  impactText: "If this is on, the run stops on the first failure.",
                  effectiveValueText: "on",
                  effectiveDisplayValueText: "on",
                }],
                effective: [],
                project: [],
                global: [],
                defaults: [],
                workers: [],
                plan: [],
                doctor: [],
                memory: [],
              },
              hasProjectConfig: true,
              hasGlobalConfig: true,
              projectPath: ".maestro/config.yaml",
              globalPath: "~/.maestro/config.yaml",
              errors: [],
            },
          }),
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "edit-inline",
            selectedScope: "global",
            draftValue: "off",
          },
        });

        const next = reduce(state, { type: "config-toggle-scope" });

        expect(next.modal.kind).toBe("config");
        if (next.modal.kind === "config") {
          expect(next.modal.phase).toBe("edit-inline");
          expect(next.modal.selectedScope).toBe("project");
        }
      });

      it("normalizes enter into global scope for global-only settings", () => {
        const state = makeState({
          snapshot: makeSnapshot({
            configInspector: {
              tabs: ["overview", "effective", "project", "global", "defaults", "workers", "plan", "doctor", "memory"],
              rowsByTab: {
                overview: [{
                  keyPath: "ui.missionControl.backgroundMode",
                  label: "Background mode",
                  section: "Quick settings",
                  valueText: "terminal",
                  displayValueText: "terminal background",
                  source: "global",
                  sourceBadge: "G",
                  editKind: "enum",
                  editKindLabel: "choice",
                  options: ["solid", "terminal"],
                  description: "Use a solid fill or let the terminal background show through.",
                  summary: "Choose whether Mission Control paints a solid backdrop.",
                  impactText: "Terminal background mode keeps normal chrome transparent.",
                  effectiveValueText: "terminal",
                  effectiveDisplayValueText: "terminal background",
                }],
                effective: [],
                project: [],
                global: [],
                defaults: [],
                workers: [],
                plan: [],
                doctor: [],
                memory: [],
              },
              hasProjectConfig: true,
              hasGlobalConfig: true,
              projectPath: ".maestro/config.yaml",
              globalPath: "~/.maestro/config.yaml",
              errors: [],
            },
          }),
          modal: {
            kind: "config",
            tab: "overview",
            selectedRowIndex: 0,
            phase: "browse",
            selectedScope: "project",
          },
        });

        const next = reduce(state, { type: "enter" });

        expect(next.modal.kind).toBe("config");
        if (next.modal.kind === "config") {
          expect(next.modal.phase).toBe("edit-inline");
          expect(next.modal.selectedScope).toBe("global");
        }
      });
    });
  });
});
