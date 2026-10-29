#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ESP32_SENSE="/Users/lodripeter/workspace/peterlodri-sec/esp32.vaked.dev/tools/aurora_sense.py"

echo "=== [1/3] Building & Signing Aurora Android APK (is.eightb.aurora) ==="
bash "$ROOT/android/build_apk.sh"
APK="$ROOT/dist/Aurora-v1.0.0-android-universal.apk"

echo "=== [2/3] Deploying Aurora to Connected ESP32-S3 & LilyGo T-Deck Serial/ESP-NOW Nodes ==="
SERIAL_PORTS=()
for p in /dev/cu.usbmodem* /dev/cu.wchusbserial* /dev/cu.SLAB_USBtoUART*; do
  [ -e "$p" ] && SERIAL_PORTS+=("$p")
done

if [ "${#SERIAL_PORTS[@]}" -eq 0 ]; then
  echo "[serial] No /dev/cu.usbmodem* or LilyGo serial ports currently enumerated."
else
  for port in "${SERIAL_PORTS[@]}"; do
    echo "[serial] Handshaking & streaming Aurora 64B@0x62 LitanyWireFrame on $port ..."
    uv run --with pyserial python "$ESP32_SENSE" \
      --port "$port" \
      --seconds 5 \
      --interval 1.5 \
      --max-lines 2 || true
  done
fi

echo "=== [3/3] Deploying Aurora APK to Connected Android Devices (ADB) ==="
ADB_SERIALS=($(adb devices | awk 'NR>1 && $2=="device" {print $1}'))
if [ "${#ADB_SERIALS[@]}" -eq 0 ]; then
  echo "[android] No ADB devices currently in 'device' state (staged $APK for hotplug)."
else
  for s in "${ADB_SERIALS[@]}"; do
    echo "[android] Installing $APK on $s ..."
    adb -s "$s" install -r "$APK"
    adb -s "$s" shell "am start -n is.eightb.aurora/.MainActivity -a android.intent.action.VIEW -d 'aurora://feel-me-tonight'"
  done
fi

echo "=== [DONE] Aurora Multi-Device Deployment Complete ==="
