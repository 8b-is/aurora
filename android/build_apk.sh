#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
BT="$SDK/build-tools/36.0.0"
PLATFORM="$SDK/platforms/android-36/android.jar"

BUILD="$HERE/build"
rm -rf "$BUILD"
mkdir -p "$BUILD/gen" "$BUILD/obj" "$BUILD/dex" "$BUILD/assets" "$ROOT/dist"

cp "$ROOT/index.html" "$BUILD/assets/index.html"

echo "[aurora-android] Linking AndroidManifest.xml + assets with aapt2..."
"$BT/aapt2" link \
  -I "$PLATFORM" \
  --manifest "$HERE/AndroidManifest.xml" \
  -A "$BUILD/assets" \
  --java "$BUILD/gen" \
  -o "$BUILD/unaligned.apk"

echo "[aurora-android] Compiling MainActivity.java -> classes.dex..."
javac -source 11 -target 11 \
  -cp "$PLATFORM" \
  -d "$BUILD/obj" \
  "$HERE/src/is/eightb/aurora/MainActivity.java"

"$BT/d8" --lib "$PLATFORM" --output "$BUILD/dex" $(find "$BUILD/obj" -name "*.class")

(cd "$BUILD/dex" && zip -q "$BUILD/unaligned.apk" classes.dex)

KEYSTORE="$HERE/debug-8b-is.keystore"
if [ ! -f "$KEYSTORE" ]; then
  keytool -genkeypair \
    -keystore "$KEYSTORE" \
    -storepass "8bisheals" \
    -keypass "8bisheals" \
    -alias "8b-is-aurora" \
    -keyalg RSA \
    -keysize 2048 \
    -validity 10000 \
    -dname "CN=8b-is AURORA, OU=Lovetta Lane, O=8b-is, L=Tokyo, ST=Tokyo, C=JP" >/dev/null 2>&1
fi

OUT_APK="$ROOT/dist/Aurora-v1.0.0-android-universal.apk"
rm -f "$OUT_APK"
"$BT/zipalign" -p -f 4 "$BUILD/unaligned.apk" "$OUT_APK"
"$BT/apksigner" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:8bisheals \
  --key-pass pass:8bisheals \
  --ks-key-alias "8b-is-aurora" \
  "$OUT_APK"

"$BT/apksigner" verify --verbose "$OUT_APK" | head -n 5
echo "[aurora-android] Built & signed APK: $OUT_APK"
