import { describe, expect, it } from "vitest";
import { clockOffset, elapsedSince, serverTime } from "./clock";

const SERVER_NOW = 1_760_000_000_000;
const RECEIVED_AT = 50_000;

describe("clock offset", () => {
  it("keeps the timer running across a remount", () => {
    // The payload arrived when the device clock read RECEIVED_AT.
    const offset = clockOffset(SERVER_NOW, RECEIVED_AT);

    // Ten minutes later the view is mounted again with the same payload.
    const later = RECEIVED_AT + 10 * 60_000;
    expect(serverTime(later, offset)).toBe(SERVER_NOW + 10 * 60_000);
  });

  it("advances one to one with the local clock", () => {
    const offset = clockOffset(9_000, 3_000);
    expect(offset).toBe(6_000);
    expect(serverTime(3_000, offset)).toBe(9_000);
    expect(serverTime(8_000, offset)).toBe(14_000);
  });

  it("does not depend on when it is evaluated", () => {
    // The bug: recomputing the offset from Date.now() at render time made it
    // shrink by the time spent in another view.
    const correct = clockOffset(SERVER_NOW, RECEIVED_AT);
    const buggy = clockOffset(SERVER_NOW, RECEIVED_AT + 600_000);
    expect(correct).not.toBe(buggy);
    expect(correct).toBe(SERVER_NOW - RECEIVED_AT);
  });

  it("measures elapsed time of a running segment", () => {
    const offset = clockOffset(SERVER_NOW, RECEIVED_AT);
    const startedAt = SERVER_NOW - 90_000;
    expect(elapsedSince(startedAt, RECEIVED_AT, offset)).toBe(90_000);
    expect(elapsedSince(startedAt, RECEIVED_AT + 60_000, offset)).toBe(150_000);
  });

  it("never reports negative elapsed time", () => {
    const offset = clockOffset(SERVER_NOW, RECEIVED_AT);
    expect(elapsedSince(SERVER_NOW + 30_000, RECEIVED_AT, offset)).toBe(0);
  });
});
