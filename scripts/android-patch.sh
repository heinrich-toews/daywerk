#!/usr/bin/env bash
# Apply the hand patches to the generated Android project.
#
# `tauri android init` regenerates `apps/desktop/src-tauri/gen/android` and
# resets everything below, so re-run this script afterwards:
#
#   scripts/android.sh init --ci && scripts/android-patch.sh
#
# 1. BuildTask.kt      the Gradle `rust` task runs `pnpm tauri …`, which needs a
#                      local CLI install. This repo uses the global CLI, so the
#                      executable becomes `tauri` and the first arg is dropped.
# 2. build.gradle.kts  the template targets SDK 37, which is not in the stable
#                      channel yet; pin 36. Also allow cleartext HTTP in release
#                      builds — self-hosted Daywerk servers are usually plain
#                      HTTP on a LAN.
# 3. MainActivity.kt   the template enables edge-to-edge but never insets the
#                      WebView, so the status bar overlaps the app header; the
#                      keyboard is an inset too and must shrink the WebView.
# 4. themes/colors     the web UI is light, so force a light window and paint
#                      the strip behind the system bars in the app background.
# 5. build.gradle.kts  release signing, keystore kept outside the repository.
# 6. AndroidManifest   resize the window for the keyboard instead of panning it,
#                      so focused login fields stay visible.
set -euo pipefail

cd "$(dirname "$0")/.."
GEN="apps/desktop/src-tauri/gen/android"

if [[ ! -d "$GEN" ]]; then
  echo "error: $GEN not found — run 'pnpm android:init --ci' first" >&2
  exit 1
fi

python3 - "$GEN" <<'PY'
import pathlib
import re
import sys

gen = pathlib.Path(sys.argv[1])
changed: list[str] = []


def patch(path: pathlib.Path, before: str, after: str, label: str, count: int = 1) -> None:
    text = path.read_text()
    if after in text:
        return
    if text.count(before) != count:
        raise SystemExit(f"error: {label}: expected {count} match(es) in {path}")
    path.write_text(text.replace(before, after))
    changed.append(label)


# 1. BuildTask.kt — call the global tauri CLI
build_task = next(gen.glob("buildSrc/src/main/java/**/BuildTask.kt"))
patch(build_task, 'val executable = """pnpm""";', 'val executable = """tauri""";', "BuildTask executable")
patch(
    build_task,
    'val args = listOf("tauri", "android", "android-studio-script");',
    'val args = listOf("android", "android-studio-script");',
    "BuildTask args",
)

# 2. app/build.gradle.kts — SDK level and cleartext traffic
gradle = gen / "app/build.gradle.kts"
patch(gradle, "compileSdk = 37", "compileSdk = 36", "compileSdk")
patch(gradle, "targetSdk = 37", "targetSdk = 36", "targetSdk")
# `optimization {` exists only in the release block, so this stays idempotent
# even after the signing patch below has inserted its own line.
patch(
    gradle,
    """            optimization {""",
    """            // Self-hosted servers are usually plain HTTP on a LAN.
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            optimization {""",
    "release cleartext",
)

# 3. MainActivity.kt — inset the WebView below the system bars
activity = next(gen.glob("app/src/main/java/**/MainActivity.kt"))
patch(
    activity,
    """import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}""",
    """import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // The template draws edge-to-edge but never insets the WebView, so the
    // status bar overlapped the app header. Pad the content by the system bars
    // and any display cutout.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
    ViewCompat.requestApplyInsets(content)
  }
}""",
    "MainActivity insets",
)

# 3b. MainActivity.kt — the keyboard is an inset too
patch(
    activity,
    """      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)""",
    """      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      // Edge-to-edge means the window no longer resizes for the keyboard
      // (adjustResize is a no-op on Android 15+), so the WebView would sit
      // behind it. Pad by whichever is larger; when the system did resize,
      // the IME inset is zero and nothing is added twice.
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))""",
    "MainActivity IME insets",
)

# 4. window colours — the web UI is light
(gen / "app/src/main/res/values/colors.xml").write_text(
    """<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="purple_200">#FFBB86FC</color>
    <color name="purple_500">#FF6200EE</color>
    <color name="purple_700">#FF3700B3</color>
    <color name="teal_200">#FF03DAC5</color>
    <color name="teal_700">#FF018786</color>
    <color name="black">#FF000000</color>
    <color name="white">#FFFFFFFF</color>
    <!-- Matches the web UI background colour. -->
    <color name="daywerk_bg">#FFF4F8F4</color>
</resources>
"""
)

theme = """<resources xmlns:tools="http://schemas.android.com/tools">
    <!-- The web UI is light, so force a light window and paint the strip behind
         the (transparent) system bars with the app background colour. -->
    <style name="Theme.daywerk_desktop" parent="Theme.Material3.Light.NoActionBar">
        <item name="android:windowBackground">@color/daywerk_bg</item>
        <item name="android:statusBarColor">@android:color/transparent</item>
        <item name="android:navigationBarColor">@android:color/transparent</item>
        <item name="android:windowLightStatusBar">true</item>
        <item name="android:windowLightNavigationBar">true</item>
    </style>
</resources>
"""
(gen / "app/src/main/res/values/themes.xml").write_text(theme)
(gen / "app/src/main/res/values-night/themes.xml").write_text(theme)


# 6. AndroidManifest — shrink the window for the on-screen keyboard
patch(
    gen / "app/src/main/AndroidManifest.xml",
    'android:configChanges="orientation',
    'android:windowSoftInputMode="adjustResize"\n            android:configChanges="orientation',
    "windowSoftInputMode",
)

# 5. release signing — keystore stays outside the repository
gradle = gen / "app/build.gradle.kts"
text = gradle.read_text()
if "signingConfigs" not in text:
    loader = '''
// Release signing: the keystore lives outside the repository, pointed to by
// DAYWERK_KEYSTORE_PROPERTIES (absolute path) or app/keystore.properties.
val daywerkKeystoreFile =
    file(System.getenv("DAYWERK_KEYSTORE_PROPERTIES") ?: "keystore.properties")
val daywerkKeystore = Properties().apply {
    if (daywerkKeystoreFile.exists()) {
        daywerkKeystoreFile.inputStream().use { load(it) }
    }
}
'''
    if "\nandroid {" not in text:
        raise SystemExit("error: cannot find the android block")
    text = text.replace("\nandroid {", "\n" + loader + "\nandroid {", 1)

    if "    buildTypes {" not in text:
        raise SystemExit("error: cannot find buildTypes")
    text = text.replace(
        "    buildTypes {",
        '''    signingConfigs {
        create("release") {
            if (daywerkKeystoreFile.exists()) {
                storeFile = file(daywerkKeystore.getProperty("storeFile"))
                storePassword = daywerkKeystore.getProperty("storePassword")
                keyAlias = daywerkKeystore.getProperty("keyAlias")
                keyPassword = daywerkKeystore.getProperty("keyPassword")
            }
        }
    }
    buildTypes {''',
        1,
    )

    marker = '''        getByName("release") {
            // Self-hosted servers are usually plain HTTP on a LAN.'''
    if marker not in text:
        raise SystemExit("error: cannot find the release build type")
    text = text.replace(
        marker,
        '''        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
            // Self-hosted servers are usually plain HTTP on a LAN.''',
        1,
    )
    gradle.write_text(text)
    changed.append("release signing")

print("patched:", ", ".join(changed) if changed else "already up to date")
PY
