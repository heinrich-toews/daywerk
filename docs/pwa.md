# PWA (browser install)

Daywerk is installable from the browser on Android and iOS. The same build is
served by the Daywerk server — there is no separate store listing and no build
step beyond `pnpm --filter web-ui build`.

## Install

**Android (Chrome):** open `https://time.hgs-t.de`, sign in, then use the
browser menu → *App installieren* (or the install icon in the address bar).
Daywerk then appears in the launcher with its own window.

**iPhone / iPad (Safari):** open the URL, tap *Teilen* → *Zum Home-Bildschirm*.
Safari does not offer a native install prompt; the home-screen entry is the
supported route.

The installed app runs standalone (no browser chrome) and keeps the same
session, so no second login is needed after installing.

## What works offline

The service worker precaches only the **app shell**: HTML, JS, CSS, icons and
the manifest. Without a connection the app opens and shows the login screen (or
the last session shell) plus an offline banner; signing in, punching time and
everything else still needs the server.

API responses, tokens and employee data are **never** cached — Workbox only
handles the precached shell and navigations, and `/api/**` is excluded from the
navigation fallback.

## Updates

`registerType: "autoUpdate"` with `skipWaiting`/`clientsClaim`: a new deployment
is picked up on the next load. The server serves `index.html` fresh and the
hashed bundles are immutable, so there is no stale-asset problem.

## Icons and branding

The install icon is static (`apps/web-ui/public/icon-192.png`,
`icon-512.png`, `icon-maskable-512.png`, `apple-touch-icon.png`) and uses the
Daywerk mark. The organisation logo uploaded in *Verwaltung → Organisation* is
shown inside the app, not as the launcher icon — a per-tenant icon would need a
per-tenant manifest, which is out of scope.

## Limits compared to the Android app

- No push notifications, no background sync.
- Haptics are not available on iOS Safari; Android Chrome supports
  `navigator.vibrate`, which the web build already uses as a fallback.
- The app is online-first; the shell is cached, the data is not.
- Installation is per device/browser profile; clearing site data removes it.

## Release artifact

`scripts/release_export.py` adds `Daywerk-<version>-<date>-pwa.zip` (the built
`apps/web-ui/dist`) next to the APK/AAB, so a release can be re-hosted or
inspected without rebuilding.
