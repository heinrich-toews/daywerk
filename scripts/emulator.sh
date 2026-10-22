#!/usr/bin/env bash
# Start the Android emulator with settings that work on this host.
#
# Why `-gpu host`: the emulator's bundled SwiftShader software renderer
# segfaults here (RenderThread -> lib64/gles_swiftshader/libGLESv2.so), which
# aborts startup ~5s in. Using the host GPU (AMD + Mesa) avoids SwiftShader.
#
# Usage: scripts/emulator.sh [AVD] [extra emulator args...]
#   scripts/emulator.sh                 # Pixel_7, windowed
#   scripts/emulator.sh Pixel_7 -no-window
set -euo pipefail

AVD="${1:-Pixel_7}"
shift || true

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-$HOME/.android/avd}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

exec "$ANDROID_HOME/emulator/emulator" -avd "$AVD" \
  -no-snapshot -no-audio -no-boot-anim -gpu host "$@"
