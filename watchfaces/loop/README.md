# Loop Launcher icon

The only maintained icon source and binary are in `crates/loop-device/assets/`.
The duplicate `appicon_loop.bin` in this legacy directory was removed in 0.3.5.
The module embeds the current icon and writes it to its private data directory before registration.

Run `sh scripts/build-wsl.sh` in WSL to generate installers under `dist/installer/`.
This directory only retains historical notes and ignored legacy artifacts.
