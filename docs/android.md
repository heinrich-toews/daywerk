# Android app (Tauri v2)

The web UI and the Rust core are shared; only the mobile shell is new. The app is
a **client**: it talks to a Daywerk server over the REST API, so it needs to know
the server address (see [Server address](#server-address)).

Verified on the `Pixel_7` AVD (Android 15, x86_64).

## Toolchain (Fedora)

| Component | Where |
|---|---|
| JDK 17 | `~/.sdkman/candidates/java/17.0.20-tem` (Fedora's Java 25 is rejected by AGP) |
| Android SDK | `~/Android/Sdk` (cmdline-tools, platform-tools, build-tools 36, platforms android-35/36, emulator) |
| NDK | `~/Android/Sdk/ndk/27.0.12077973` |
| Rust | `rustup` with `aarch64-linux-android`, `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android` |
| Tauri CLI | 2.x (`tauri --version`) |

Everything is already installed on this machine. On a new one:

```sh
curl -s "https://get.sdkman.io" | bash
source ~/.sdkman/bin/sdkman-init.sh
sdk install java 17.0.20-tem

sdkmanager --install "platform-tools" "build-tools;36.0.0" "platforms;android-36" "emulator"
sdkmanager --install "ndk;27.0.12077973"
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

`scripts/android.sh` exports `JAVA_HOME`, `ANDROID_HOME`, `NDK_HOME` and `PATH`
for every build, so the commands below just work.

## Build & run

```sh
# emulator (x86_64)
VITE_SERVER_URL=http://10.0.2.2:9095 pnpm android:build:debug --target x86_64

# physical phone (arm64)
VITE_SERVER_URL=https://time.example.org pnpm android:build --target aarch64
```

The APK lands in
`apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/`.

```sh
adb install -r apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
adb shell am start -n de.twxsoft.daywerk/.MainActivity
```

The package id is `de.twxsoft.daywerk`.

## Server address

Inside the Tauri shell the page is served from `http://tauri.localhost`, so a
relative `/api/...` request would go nowhere. Three ways to point the app at a
server, in order of usefulness:

1. **Build time** — `VITE_SERVER_URL` is baked in (used above). Best for a fixed
   deployment or a test build.
2. **Login screen** — the app shows a *Server-Adresse* field when no address is
   configured. The value is stored in `localStorage` and can be changed later in
   *Einstellungen → Server*.
3. **`adb reverse`** — map the host port into the device and use
   `http://localhost:9095`:

   ```sh
   adb reverse tcp:9095 tcp:9095
   ```

For the emulator, `http://10.0.2.2:9095` reaches the host's loopback. A physical
phone needs the host's LAN address or the public URL.

Debug builds allow cleartext HTTP; the release build is patched to allow it too
because self-hosted Daywerk servers are usually plain HTTP on a LAN.

## Daily dev loop

```sh
pnpm android:emulator                                   # Pixel_7, -gpu host
VITE_SERVER_URL=http://10.0.2.2:9095 \
  pnpm android:build:debug --target x86_64                # emulator build
adb install -r apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
adb shell am start -n de.twxsoft.daywerk/.MainActivity

# hot reload against a running device/emulator (dev server on the host)
pnpm android:dev
```

## Generated project is patched

`tauri android init` regenerates `apps/desktop/src-tauri/gen/android` and resets
the hand patches. After re-running it, apply them again:

```sh
pnpm android:init --ci && scripts/android-patch.sh
```

`scripts/android-patch.sh` is idempotent and does four things:

1. **`BuildTask.kt`** — the Gradle `rust` task runs `pnpm tauri …`, which needs a
   local CLI install. This repo uses the global CLI, so the executable becomes
   `tauri` and the first argument is dropped.
2. **`app/build.gradle.kts`** — `compileSdk`/`targetSdk` pinned from 37 to **36**
   (37 is not in the stable SDK repo), and the release build gets
   `usesCleartextTraffic = true`.
3. **`MainActivity.kt`** — applies system bar and display-cutout insets to the
   content view. The template calls `enableEdgeToEdge()` but never insets the
   WebView, so the status bar overlapped the app header.
4. **`themes.xml` / `colors.xml`** — the web UI is light, so force a light window
   and paint the strip behind the bars in the app background colour. XML comments
   must not contain `--`.

## Notes / gotchas

- **Always use `-gpu host` on this host.** The bundled SwiftShader renderer
  segfaults in `RenderThread` a few seconds after start.
- The emulator is x86_64 → build with `--target x86_64`. A phone wants `aarch64`.
- Debug APKs are large (~250 MB) because they keep debug symbols; release builds
  are far smaller.
- The first Rust cross-compile takes several minutes; later builds are cached.
- Run every build as the desktop user (not root): the pnpm store, `node_modules`
  and `~/.cargo` must be writable, otherwise Vite fails with `EACCES` while
  emptying `dist/`.
- WebView devtools: open `chrome://inspect` in Chrome on the host and select the
  `de.twxsoft.daywerk` webview.
