#!/bin/sh
# 构建含 036 + 043 双 payload 的 Loop 生产安装表盘。
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CANOPUS=${CANOPUS_ROOT:-"$ROOT/../Canopus"}
WATCHFACE=${CANOPUS_WATCHFACE_OUT:-"$ROOT/watchfaces/loop-prod"}
mkdir -p "$WATCHFACE"

set -- \
  xiaomi-band-10-pro-3.101.036 \
  xiaomi-band-10-pro-3.101.043
for TARGET_ID do
  rm -f "$WATCHFACE/loop-$TARGET_ID.bin"
  rm -f "$WATCHFACE/loop-$TARGET_ID.cmi.bin"
done
cp "$ROOT/watchfaces/loop/appicon_loop.bin" "$WATCHFACE/appicon_loop.bin"
[ -f "$WATCHFACE/main.lua" ] || {
    echo "error: missing $WATCHFACE/main.lua (multi-target installer)" >&2
    exit 1
}

cargo fmt --manifest-path "$ROOT/Cargo.toml" --all -- --check
if command -v luac >/dev/null 2>&1; then
    luac -p "$WATCHFACE/main.lua"
fi

for TARGET_ID do
  OUT="$ROOT/build/loop-prod/$TARGET_ID"
  STAGE="$OUT/watchface"
  rm -rf "$STAGE"
  mkdir -p "$STAGE"
  cp "$ROOT/watchfaces/loop/main.lua" "$STAGE/main.lua"
  cp "$ROOT/watchfaces/loop/appicon_loop.bin" "$STAGE/appicon_loop.bin"
  CANOPUS_TARGET="$TARGET_ID" \
  CANOPUS_BUILD_OUT="$OUT" \
  CANOPUS_WATCHFACE_OUT="$STAGE" \
    "$ROOT/scripts/build-install-watchface.sh"
  cp "$STAGE/module.bin" "$WATCHFACE/loop-$TARGET_ID.bin"
  cp "$STAGE/receipt.bin" "$WATCHFACE/loop-$TARGET_ID.cmi.bin"
  rm -rf "$STAGE"
done

python3 - "$ROOT" "$CANOPUS" "$WATCHFACE" \
  xiaomi-band-10-pro-3.101.036 \
  xiaomi-band-10-pro-3.101.043 <<'PY'
import hashlib
import pathlib
import struct
import sys
import tomllib

root = pathlib.Path(sys.argv[1])
canopus = pathlib.Path(sys.argv[2])
watchface = pathlib.Path(sys.argv[3])
targets = sys.argv[4:]
module_digests = set()

for target in targets:
    stem = watchface / f"loop-{target}"
    module_path = pathlib.Path(str(stem) + ".bin")
    receipt_path = pathlib.Path(str(stem) + ".cmi.bin")
    module = module_path.read_bytes()
    receipt = receipt_path.read_bytes()
    assert 512 <= len(module) <= 393216
    assert module[:7] == b"\x7fELF\x01\x01\x01"
    assert struct.unpack_from("<HH", module, 16) == (1, 40)
    assert len(receipt) == 256 and receipt[:4] == b"CMI1"
    magic, version, header, _flags, lifecycle, module_version, artifact_size, _reserved = struct.unpack(
        "<8I", receipt[:32]
    )
    assert magic == 0x31494D43 and version == 1 and header == 256
    assert lifecycle in range(4) and module_version == 1
    assert artifact_size == len(module), (target, artifact_size, len(module))
    module_id = receipt[32:64].split(b"\0", 1)[0]
    receipt_target = receipt[64:112].split(b"\0", 1)[0].decode("ascii")
    receipt_firmware = receipt[112:144].hex()
    profile = tomllib.loads((canopus / "targets" / target / "target.toml").read_text(encoding="utf-8"))
    assert profile["target_id"] == target
    assert module_id == b"loop"
    assert receipt_target == target, (receipt_target, target)
    assert receipt_firmware == profile["firmware_sha256"]
    digest = hashlib.sha256(module).digest()
    assert receipt[144:176] == digest
    assert digest not in module_digests
    module_digests.add(digest)
icon = (watchface / "appicon_loop.bin").read_bytes()
assert len(icon) == 54768
print(f"loop-prod ready: {len(targets)} targets")
PY
printf '%s\n' "watchfaces/loop-prod is ready to install"
