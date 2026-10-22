/**
 * Server/device clock alignment.
 *
 * The API sends `now` with every payload. The offset between the server clock
 * and this device must be derived from the moment that payload *arrived* and
 * must never be recomputed from the current time — otherwise every remount
 * shifts the offset and a running timer jumps backwards.
 */

/** Offset between server clock and device clock, in milliseconds. */
export function clockOffset(serverNow: number, receivedAt: number): number {
  return serverNow - receivedAt;
}

/** What the server clock reads now, given a local clock reading. */
export function serverTime(localNow: number, offset: number): number {
  return localNow + offset;
}

/** Elapsed milliseconds of a segment that started at `startedAt`. */
export function elapsedSince(startedAt: number, localNow: number, offset: number): number {
  return Math.max(0, serverTime(localNow, offset) - startedAt);
}
