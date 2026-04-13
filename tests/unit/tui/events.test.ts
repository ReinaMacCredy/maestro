import { describe, expect, it } from "bun:test";
import { deriveEvents } from "@/tui/state/events.js";
import type { Mission, Feature, Checkpoint, Assertion } from "@/features/mission";

function makeMission(overrides: Partial<Mission> = {}): Mission {
  return {
    id: "2026-03-30-001",
    status: "executing",
    title: "Test Mission",
    description: "A test mission",
    milestones: [{ id: "m1", title: "Milestone 1", description: "First", order: 0, featureIds: [] }],
    features: ["f1"],
    createdAt: "2026-03-30T10:00:00.000Z",
    updatedAt: "2026-03-30T10:05:00.000Z",
    ...overrides,
  };
}

function makeFeature(overrides: Partial<Feature> = {}): Feature {
  return {
    id: "f1",
    missionId: "2026-03-30-001",
    milestoneId: "m1",
    status: "pending",
    title: "Feature 1",
    description: "First feature",
    workerType: "test-skill",
    verificationSteps: [],
    dependsOn: [],
    fulfills: [],
    createdAt: "2026-03-30T10:01:00.000Z",
    updatedAt: "2026-03-30T10:01:00.000Z",
    ...overrides,
  };
}

describe("deriveEvents", () => {
  it("produces mission created event", () => {
    const events = deriveEvents({
      mission: makeMission(),
      features: [],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    const created = events.find((e) => e.title === "Mission created");
    expect(created).toBeDefined();
    expect(created!.kind).toBe("mission");
  });

  it("produces mission approved event when approvedAt is set", () => {
    const events = deriveEvents({
      mission: makeMission({ approvedAt: "2026-03-30T10:02:00.000Z" }),
      features: [],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    const approved = events.find((e) => e.title === "Mission approved");
    expect(approved).toBeDefined();
  });

  it("produces feature created event", () => {
    const events = deriveEvents({
      mission: makeMission(),
      features: [makeFeature()],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    const fCreated = events.find((e) => e.title === "f1 created");
    expect(fCreated).toBeDefined();
    expect(fCreated!.kind).toBe("feature");
  });

  it("produces feature status change event", () => {
    const events = deriveEvents({
      mission: makeMission(),
      features: [makeFeature({
        status: "in-progress",
        updatedAt: "2026-03-30T10:10:00.000Z",
      })],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    const statusChange = events.find((e) => e.title === "f1 moved to in-progress");
    expect(statusChange).toBeDefined();
  });

  it("does not produce status change for pending features", () => {
    const events = deriveEvents({
      mission: makeMission(),
      features: [makeFeature({ status: "pending" })],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    const statusChange = events.find((e) => e.title.includes("moved to"));
    expect(statusChange).toBeUndefined();
  });

  it("produces assertion events for non-pending results", () => {
    const assertion: Assertion = {
      id: "a1",
      missionId: "2026-03-30-001",
      milestoneId: "m1",
      featureId: "f1",
      result: "passed",
      description: "Test assertion",
      surface: "cli",
      createdAt: "2026-03-30T10:01:00.000Z",
      updatedAt: "2026-03-30T10:15:00.000Z",
    };

    const events = deriveEvents({
      mission: makeMission(),
      features: [],
      assertions: [assertion],
      checkpoints: [],
      milestoneProgress: [],
    });

    const aEvent = events.find((e) => e.title === "a1: passed");
    expect(aEvent).toBeDefined();
    expect(aEvent!.kind).toBe("assertion");
  });

  it("produces checkpoint events", () => {
    const checkpoint: Checkpoint = {
      id: "cp-001",
      missionId: "2026-03-30-001",
      currentMilestoneId: "m1",
      timestamp: "2026-03-30T10:12:00.000Z",
      featureStatuses: {},
      assertionResults: {},
    };

    const events = deriveEvents({
      mission: makeMission(),
      features: [],
      assertions: [],
      checkpoints: [checkpoint],
      milestoneProgress: [],
    });

    const cpEvent = events.find((e) => e.title === "Checkpoint saved: cp-001");
    expect(cpEvent).toBeDefined();
    expect(cpEvent!.kind).toBe("checkpoint");
  });

  it("returns events sorted by timestamp descending", () => {
    const events = deriveEvents({
      mission: makeMission({ approvedAt: "2026-03-30T10:02:00.000Z" }),
      features: [makeFeature({
        status: "done",
        updatedAt: "2026-03-30T10:20:00.000Z",
      })],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    for (let i = 0; i < events.length - 1; i++) {
      expect(events[i]!.timestamp >= events[i + 1]!.timestamp).toBe(true);
    }
  });

  it("empty mission produces only Mission created event", () => {
    const events = deriveEvents({
      mission: makeMission(),
      features: [],
      assertions: [],
      checkpoints: [],
      milestoneProgress: [],
    });

    expect(events.length).toBe(1);
    expect(events[0]!.title).toBe("Mission created");
  });
});
