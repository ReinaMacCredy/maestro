import { describe, it, expect } from "bun:test";
import {
  compareRiskClass,
  maxRiskClass,
  RISK_CLASS_ORDER,
} from "@/features/risk/usecases/risk-class-order.js";

describe("RISK_CLASS_ORDER", () => {
  it("has four levels in ascending order", () => {
    expect(RISK_CLASS_ORDER).toEqual(["low", "medium", "high", "critical"]);
  });
});

describe("compareRiskClass", () => {
  it("returns -1 when a < b", () => {
    expect(compareRiskClass("low", "medium")).toBe(-1);
    expect(compareRiskClass("low", "high")).toBe(-1);
    expect(compareRiskClass("low", "critical")).toBe(-1);
    expect(compareRiskClass("medium", "high")).toBe(-1);
    expect(compareRiskClass("medium", "critical")).toBe(-1);
    expect(compareRiskClass("high", "critical")).toBe(-1);
  });

  it("returns 0 when a === b", () => {
    expect(compareRiskClass("low", "low")).toBe(0);
    expect(compareRiskClass("medium", "medium")).toBe(0);
    expect(compareRiskClass("high", "high")).toBe(0);
    expect(compareRiskClass("critical", "critical")).toBe(0);
  });

  it("returns 1 when a > b", () => {
    expect(compareRiskClass("medium", "low")).toBe(1);
    expect(compareRiskClass("high", "medium")).toBe(1);
    expect(compareRiskClass("critical", "high")).toBe(1);
    expect(compareRiskClass("critical", "low")).toBe(1);
  });

  it("is transitive: low < medium < high < critical", () => {
    expect(compareRiskClass("low", "medium")).toBe(-1);
    expect(compareRiskClass("medium", "high")).toBe(-1);
    expect(compareRiskClass("high", "critical")).toBe(-1);
  });
});

describe("maxRiskClass", () => {
  it("returns the higher class", () => {
    expect(maxRiskClass("low", "critical")).toBe("critical");
    expect(maxRiskClass("critical", "low")).toBe("critical");
    expect(maxRiskClass("medium", "high")).toBe("high");
    expect(maxRiskClass("high", "medium")).toBe("high");
  });

  it("returns the same class when equal", () => {
    expect(maxRiskClass("high", "high")).toBe("high");
    expect(maxRiskClass("low", "low")).toBe("low");
  });
});
