# Loop installer watchface (single target)

Build with `scripts/build-install-watchface.sh`. Opening the watchface installs
the signed `loop` module through an already-present `/dev/canopus` supervisor.

Tracked assets:

- `main.lua` — installer UI
- `appicon_loop.bin` — launcher icon (placeholder LVGL v9 BIN; replace for release)

Generated (gitignored): `module.bin`, `receipt.bin`
