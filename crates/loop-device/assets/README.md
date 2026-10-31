# Launcher 图标

- `appicon_loop.png`：117×117 RGBA 源图。
- `appicon_loop.bin`：LVGL v9 ARGB8888 图标，由设备模块内嵌，在发布应用前写入模块私有目录。

在 WSL 中，从 Loop 根目录重新生成：

```sh
python3 scripts/encode-lvgl-icon.py crates/loop-device/assets/appicon_loop.png crates/loop-device/assets/appicon_loop.bin
```

设备代码使用 4 字节对齐存储。更新图标后须重新构建并通过官方 ELF verifier；可能被识别为地址的像素常量只在有证据时逐项声明于 `Canopus.toml`。

2026-10-31 从旧表盘目录迁入，PNG 重新编码与原 BIN 逐字节一致，SHA-256 为 `03bffccb1cfef8186c3cd1dede1588f304ac3d395aa19e56e0993bee05f4ddf4`。
