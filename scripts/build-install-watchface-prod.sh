#!/bin/sh
# 按设备各打一份已签名的 Loop 生产安装表盘，并按精确固件版本选择 payload。
# 用法: scripts/build-install-watchface-prod.sh [device-or-target ...]
# CANOPUS_DEVICE 优先于 CANOPUS_TARGET；命令行参数覆盖二者。
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CANOPUS=${CANOPUS_ROOT:-"$ROOT/../Canopus"}
SELECTION=${CANOPUS_DEVICE:-${CANOPUS_TARGET:-"xiaomi-band-10-pro xiaomi-band-11"}}
if [ "$#" -gt 0 ]; then SELECTION="$*"; fi
TARGET_IDS=""
for ITEM in $SELECTION; do
  case "$ITEM" in
    xiaomi-band-10-pro) EXPANDED="xiaomi-band-10-pro-3.101.036 xiaomi-band-10-pro-3.101.043" ;;
    xiaomi-band-11) EXPANDED="xiaomi-band-11-4.100.139 xiaomi-band-11-4.100.155" ;;
    xiaomi-band-10-pro-3.101.036|xiaomi-band-10-pro-3.101.043|xiaomi-band-11-4.100.139|xiaomi-band-11-4.100.155) EXPANDED="$ITEM" ;;
    *) echo "unsupported prod device/target: $ITEM" >&2; exit 1 ;;
  esac
  for TARGET_ID in $EXPANDED; do
    case " $TARGET_IDS " in *" $TARGET_ID "*) ;; *) TARGET_IDS="$TARGET_IDS $TARGET_ID" ;; esac
  done
done

WATCHFACE=${CANOPUS_WATCHFACE_OUT:-"$ROOT/watchfaces/loop-prod"}
PAYLOADS="$ROOT/build/loop-prod"
cargo fmt --manifest-path "$ROOT/Cargo.toml" --all -- --check
cargo test --manifest-path "$ROOT/Cargo.toml" -p loop-core
for TARGET_ID in $TARGET_IDS; do
  OUT="$PAYLOADS/$TARGET_ID"
  STAGE="$OUT/watchface"
  mkdir -p "$STAGE"
  cp "$ROOT/watchfaces/loop/main.lua" "$STAGE/main.lua"
  cp "$ROOT/watchfaces/loop/appicon_loop.bin" "$STAGE/appicon_loop.bin"
  CANOPUS_TARGET="$TARGET_ID" \
  CANOPUS_BUILD_OUT="$OUT" \
  CANOPUS_WATCHFACE_OUT="$STAGE" \
    "$ROOT/scripts/build-install-watchface.sh"
done

set -- --product loop --payload-dir "$PAYLOADS" \
  --assets-dir "$ROOT/watchfaces/loop" --output-dir "$WATCHFACE"
for TARGET_ID in $TARGET_IDS; do set -- "$@" --target "$TARGET_ID"; done
CANOPUS_ROOT="$CANOPUS" python3 "$ROOT/scripts/stage-loop-prod.py" "$@"
for TARGET_ID in $TARGET_IDS; do
  DEVICE=${TARGET_ID%-*}
  if command -v luac >/dev/null 2>&1; then
    luac -p "$WATCHFACE/$DEVICE/main.lua"
  fi
done
printf '%s\n' "$WATCHFACE is ready to package (Band 11 device display validation pending)"
