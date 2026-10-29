// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

/**
 * Keep the focused field above the on-screen keyboard.
 *
 * The native shell shrinks the WebView when the keyboard opens (the activity
 * pads for the IME inset), and the layout scrolls. This scrolls the field into
 * the remaining space once the keyboard animation has settled — without it the
 * field is visible but often at the very edge, half covered by the IME.
 */
export function installKeyboardHandling() {
  const onFocus = (event: FocusEvent) => {
    const target = event.target;
    if (
      !(
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      )
    ) {
      return;
    }
    window.setTimeout(() => {
      target.scrollIntoView({ block: "center", behavior: "smooth" });
    }, 260);
  };

  document.addEventListener("focusin", onFocus);
  return () => document.removeEventListener("focusin", onFocus);
}
