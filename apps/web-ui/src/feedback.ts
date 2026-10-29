// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Light feedback for the punch buttons: a haptic tap and an optional sound.
//!
//! Preferences are per device, so they live in localStorage rather than in the
//! account: someone may want the phone to buzz but the office laptop silent.

import { createSignal } from "solid-js";
import { isTauri } from "./api";

const VIBRATE_KEY = "daywerk.feedback.vibrate";
const SOUND_KEY = "daywerk.feedback.sound";

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const value = localStorage.getItem(key);
    return value === null ? fallback : value === "1";
  } catch {
    return fallback;
  }
}

function writeFlag(key: string, on: boolean) {
  try {
    localStorage.setItem(key, on ? "1" : "0");
  } catch {
    /* private mode */
  }
}

const [vibrate, setVibrateState] = createSignal(readFlag(VIBRATE_KEY, true));
const [sound, setSoundState] = createSignal(readFlag(SOUND_KEY, false));

/** Vibration is on by default; it is the feedback that works with the phone in a pocket. */
export const vibrationEnabled = vibrate;

/** Sound is opt-in: a row of phones beeping in an office is not a feature. */
export const soundEnabled = sound;

export function setVibration(on: boolean) {
  setVibrateState(on);
  writeFlag(VIBRATE_KEY, on);
}

export function setSound(on: boolean) {
  setSoundState(on);
  writeFlag(SOUND_KEY, on);
  if (on) tick();
}

let audio: AudioContext | null = null;

/** A quiet, short blip. Synthesised, so no asset has to ship with the app. */
function tick() {
  if (!sound()) return;
  try {
    audio ??= new AudioContext();
    void audio.resume();
    const now = audio.currentTime;
    const osc = audio.createOscillator();
    const gain = audio.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(880, now);
    gain.gain.setValueAtTime(0.0001, now);
    gain.gain.exponentialRampToValueAtTime(0.06, now + 0.005);
    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.06);
    osc.connect(gain).connect(audio.destination);
    osc.start(now);
    osc.stop(now + 0.07);
  } catch {
    /* audio unavailable: stay silent */
  }
}

async function impact(style: "light" | "medium") {
  if (isTauri) {
    try {
      const { impactFeedback } = await import("@tauri-apps/plugin-haptics");
      await impactFeedback(style);
      return;
    } catch {
      /* fall through to the web API */
    }
  }
  if (typeof navigator !== "undefined" && "vibrate" in navigator) {
    navigator.vibrate(style === "medium" ? 20 : 10);
  }
}

/** Fire feedback for a punch action. Never throws, never blocks the request. */
export function tap(style: "light" | "medium" = "light") {
  if (vibrate()) void impact(style);
  tick();
}
