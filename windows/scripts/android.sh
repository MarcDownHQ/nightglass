#!/bin/sh
# Builds (and installs) the Android TV app. Usage, from windows/:
#   sh scripts/android.sh build [armv7|aarch64]     signed APK -> apk/nightglass-<abi>.apk   (default: armv7, 32-bit boxes)
#   sh scripts/android.sh install [armv7|aarch64]   build, then `adb install -r` on the connected device
#   sh scripts/android.sh check                     only report what is missing
# Needs: JDK 17, the Android SDK with NDK 27.2.12479018 and platform 36, rustup, node, adb. Looks in the usual places;
# JAVA_HOME, ANDROID_HOME and NDK_HOME override.
set -eu
cd "$(dirname "$0")/.."
cmd=${1:-build}
abi=${2:-armv7}
case "$abi" in armv7) triple=armv7-linux-androideabi; clang=armv7a-linux-androideabi ;; aarch64) triple=aarch64-linux-android; clang=aarch64-linux-android ;; *) echo "abi: armv7 or aarch64"; exit 1 ;; esac

fail() { echo "missing: $1"; echo "  -> $2"; missing=1; }
missing=0

: "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
[ -d "$ANDROID_HOME" ] || [ ! -d "$HOME/Android/Sdk" ] || ANDROID_HOME=$HOME/Android/Sdk
: "${NDK_HOME:=$ANDROID_HOME/ndk/27.2.12479018}"
if [ -z "${JAVA_HOME:-}" ]; then
  for j in "$HOME"/android-tools/jdk-17*/Contents/Home /Library/Java/JavaVirtualMachines/*17*/Contents/Home /usr/lib/jvm/*17*; do [ -x "$j/bin/java" ] && JAVA_HOME=$j && break; done
fi
export ANDROID_HOME NDK_HOME JAVA_HOME="${JAVA_HOME:-}"
export PATH="$HOME/.cargo/bin:$ANDROID_HOME/platform-tools:$PATH"

[ -x "${JAVA_HOME:-/nonexistent}/bin/java" ] || fail "JDK 17" "install Temurin 17 (adoptium.net) and set JAVA_HOME"
[ -d "$NDK_HOME" ] || fail "NDK 27.2.12479018 in $NDK_HOME" "sdkmanager 'ndk;27.2.12479018'"
[ -d "$ANDROID_HOME/platforms/android-36" ] || fail "Android platform 36" "sdkmanager 'platforms;android-36' 'build-tools;36.0.0' 'platform-tools'"
command -v rustup >/dev/null || fail "rustup" "https://rustup.rs"
command -v node >/dev/null || fail "node" "install Node 24"
[ "$missing" = 0 ] || exit 1

# The OpenSSL that reqwest builds with the NDK looks for <triple>-ar and <triple>-ranlib, which the NDK names llvm-ar and llvm-ranlib.
bin=$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/*/bin)
shim=$PWD/target/ndk-shims
mkdir -p "$shim"
for t in arm-linux-androideabi armv7a-linux-androideabi aarch64-linux-android; do
  ln -sf "$bin/llvm-ar" "$shim/$t-ar"
  ln -sf "$bin/llvm-ranlib" "$shim/$t-ranlib"
done
export PATH="$shim:$PATH"
echo "ok: JDK $JAVA_HOME, SDK $ANDROID_HOME, NDK $NDK_HOME"
[ "$cmd" = check ] && exit 0

rustup target add "$triple" >/dev/null
[ -d node_modules ] || npm ci
# Release build (stripped, ~60 MB instead of ~350 MB), signed with the debug key so that it installs: not for a store.
npm run tauri android build -- --target "$abi" --apk
unsigned=$(ls src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk | head -1)
key=$HOME/.android/debug.keystore
if [ ! -f "$key" ]; then
  mkdir -p "$HOME/.android"
  keytool -genkeypair -keystore "$key" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi
apksigner=$(ls "$ANDROID_HOME"/build-tools/*/apksigner | sort | tail -1)
mkdir -p apk
"$apksigner" sign --ks "$key" --ks-pass pass:android --key-pass pass:android --ks-key-alias androiddebugkey --out "apk/nightglass-$abi.apk" "$unsigned"
echo "built: apk/nightglass-$abi.apk"
if [ "$cmd" = install ]; then
  adb devices
  adb install -r "apk/nightglass-$abi.apk"
fi
