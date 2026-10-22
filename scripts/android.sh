#!/usr/bin/env bash
# Run a `tauri android <cmd>` with the Fedora Android toolchain configured.
#
# Usage: scripts/android.sh <init|dev|build|run> [args...]
#   scripts/android.sh build --debug --apk --target x86_64   # emulator
#   scripts/android.sh build --debug --apk --target aarch64  # phone
#
# Toolchain (override via env if yours differs):
#   - JDK 17        from SDKMAN (Fedora ships Java 25, which AGP rejects)
#   - Android SDK   at $HOME/Android/Sdk
#   - NDK           at $ANDROID_HOME/ndk/<ver>
#   - Rust targets  via rustup (aarch64/armv7/i686/x86_64-linux-android)
set -euo pipefail

cmd="${1:?usage: android.sh <init|dev|build|run> [args...]}"
shift

export JAVA_HOME="${JAVA_HOME:-$HOME/.sdkman/candidates/java/17.0.20-tem}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/27.0.12077973}"
export PATH="$HOME/.cargo/bin:/usr/local/sbin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

cd "$(dirname "$0")/.."
exec tauri android "$cmd" "$@" -c apps/desktop/src-tauri/tauri.conf.json
