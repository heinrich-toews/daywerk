import { describe, expect, it } from "vitest";
import { addDuration, toMinutes } from "./views/tasks/util";

describe("duration chips", () => {
  it("adds to the current value", () => {
    expect(addDuration(null, 15)).toBe(15);
    expect(addDuration(15, 45)).toBe(60);
    expect(addDuration(60, 60)).toBe(120);
  });

  it("builds 2:45 from +1h, +1h, +45", () => {
    let value: number | null = null;
    value = addDuration(value, 60);
    value = addDuration(value, 60);
    value = addDuration(value, 45);
    expect(value).toBe(165);
  });

  it("never exceeds the cap", () => {
    expect(addDuration(23 * 60 + 45, 60)).toBe(24 * 60);
    expect(addDuration(24 * 60, 15)).toBe(24 * 60);
  });
});

describe("duration fields", () => {
  it("converts hours and minutes", () => {
    expect(toMinutes(1, 30)).toBe(90);
    expect(toMinutes(0, 45)).toBe(45);
    expect(toMinutes(2, 0)).toBe(120);
  });

  it("rolls minutes over and clamps the total to 24h", () => {
    expect(toMinutes(1, 75)).toBe(135); // 1 h 75 min rolls over to 2:15
    expect(toMinutes(30, 0)).toBe(24 * 60);
    expect(toMinutes(-1, 30)).toBe(30);
  });
});
