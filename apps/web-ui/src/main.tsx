import { render } from "solid-js/web";
import App from "./App";
import { isTauri } from "./api";
import { installKeyboardHandling } from "./keyboard";
import "./index.css";

installKeyboardHandling();

// The browser build is installable and caches its shell. The native shells
// load from tauri.localhost and must never register a service worker.
if (!isTauri && "serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/sw.js").catch(() => {
      /* without the cache the app still works, just not offline */
    });
  });
}

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
render(() => <App />, root);
