# Loop 构建说明（WSL）

本文是构建速查。完整环境搭建与日常开发过程见 [WSL-DEVELOPMENT.md](WSL-DEVELOPMENT.md)，参考应用结构差异见 [LYRA-NG-COMPARISON.md](LYRA-NG-COMPARISON.md)。

当前使用 Canopus `ds-newdevice-supports@c5573a69`，Loop 为 0.3.5 / versionCode 8，仅打包 `.043` / `.155` / `.179` 三个目标。v1 文案、按钮和原生页面恢复见 [V1-PARITY-034.md](V1-PARITY-034.md)，此前分支适配记录见 [DS-NEWDEVICE-SUPPORTS.md](DS-NEWDEVICE-SUPPORTS.md)。

0.3.5 的旧点击校验、切页失败恢复和旧文件清理见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)。

## 环境

所有编译、测试和打包都在 WSL2 Ubuntu 中运行，使用 Linux 侧工具链。
仓库互为兄弟目录：`/mnt/d/Canopus/Canopus`、`/mnt/d/Canopus/class-loop`。

需要 Rust stable（至少 1.88）、`thumbv8m.main-none-eabi`、clang/lld/llvm、Python 3.11+ 和 Lua 5.4（`luac`）。本次验证使用 Ubuntu 24.04、Rust 1.99.1、LLVM 18。

```sh
sudo apt-get update
sudo apt-get install build-essential clang lld llvm python3 lua5.4 pkg-config libssl-dev
rustup target add thumbv8m.main-none-eabi
export CANOPUS_ROOT=/mnt/d/Canopus/Canopus
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
export PATH="$CARGO_TARGET_DIR/debug:$PATH"
cd "$CANOPUS_ROOT"
cargo build -p canopus-cli
canopus doctor
```

`scripts/build-wsl.sh` 会加载 Linux Cargo 环境，并自动设置相邻 SDK 路径、主机 Cargo 缓存和 CLI 路径。默认先从同一个 Canopus checkout 编译 CLI，避免切换 SDK 分支后误用旧 CLI。可通过 `CANOPUS_ROOT`、`CARGO_TARGET_DIR` 和 `CANOPUS_CLI` 覆盖默认值；显式提供 `CANOPUS_CLI` 时由调用方维护它与 SDK 的匹配。Shell 文件须保持 LF 换行。

## 构建与打包

```sh
cd /mnt/d/Canopus/class-loop
sh scripts/build-wsl.sh                                   # .043 + .155 + .179
sh scripts/build-wsl.sh xiaomi-band-11                     # .155 + .179
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043         # 单固件
```

脚本依次调用官方 `canopus build`、`canopus package --installer`、`canopus package verify`，并对生成的安装器运行 `luac -p`。部分目标使用项目根目录中的临时清单，构建后自动移除。

- `build/<target>/loop.elf`：通过官方 verifier 的 ELF32 ET_REL 模块。
- `dist/loop-0.3.5.canopus`：包含全部三个目标；部分构建写入 `dist/selected/<版本组合>/`，包名带版本后缀。
- 同名 `.build-info.json`：本次 SDK 分支、完整 commit、tracked dirty 状态、CLI/包/各目标 ELF 的 SHA-256。
- `dist/installer/<设备族>/`：该次选择的固件对应的 `main.lua`、模块和 CMI2 回执。打包此目录中的 `main.lua` 和 `.bin` 资源作为安装表盘。

完整包与部分包的安装器输出目录分开，避免互相覆盖。
已自行设置上述环境变量时，也可直接调用 `python3 scripts/build.py`。旧表盘构建入口、重复校验脚本和 `targets/*.env` 已移除，清理与交互修复记录见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)。目标选择以 `Canopus.toml` 为准。安装器只写入 `dist/installer/` 或部分目标包对应的 `installer/`。

可手动检查包：

```sh
canopus package verify dist/loop-0.3.5.canopus
```

## 安装与运行

手机插件单独在 WSL 构建：

```sh
cd /mnt/d/Canopus/class-loop/plugins/astrobox-loop-import
rustup target add wasm32-wasip2
cargo test --target x86_64-unknown-linux-gnu
python3 scripts/build_dist.py --release --package
```

需要先加载 Linux Cargo 环境，并设置上述 `CARGO_TARGET_DIR`。插件产物在该目录的 `dist/`，包括 Wasm 和 `.abp`；需要支持 CanopusConnect 的 AstroBox API Level 4。详见 [插件说明](../plugins/astrobox-loop-import/README.md)。

先安装匹配精确固件的当前 ABI 2 Canopus Supervisor。旧 Supervisor、CMI1 回执和旧 registry 与本版本不兼容。
新版采用 176 字节 CMI2 回执，无签名或私钥步骤。安装器写入 `.cmi.next` / `.ko.next` inbox 暂存文件，由 Supervisor 验证和安装；升级结果 `REBOOT_REQUIRED(8)` 表示重启后生效。

Loop 通过服务表使用文件、内存、时间和 Launcher，应用 ID 由 Supervisor 分配；五个原生页面通过已有 target SDK 创建系统标题栏、列表和滚动容器，使用固件 push/pop。真实日历时间使用精确 target SDK 的 CLOCK_REALTIME，避免本分支 realtime_ms 返回开机时长。模块保持驻留，注销需要重启。
图标随 ELF 内嵌，注册前写入模块私有目录。课表由 AstroBox 插件通过 CanopusConnect 直接推送，模块校验后原子保存至私有 `schedule.json`。旧导入快应用已移除；旧沙箱课表需重新推送。当前仅打包支持 CanopusConnect 的 `.043` / `.155` / `.179`。

## 当前验证范围

0.3.5 主机测试执行实际原生描述符和系统组件渲染源码，核对全部 v1 按钮、每个星期、周次提示布局、原生 push/pop 调用、灭屏恢复、按住期间导入和切页失败恢复。覆盖 Band 10 Pro 与 Band 11 返回 ABI；真实固件绘制和动画效果仍需验收，最新结果见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)。

下文 2026-10-31 的 LVGL 测试与预览属于旧版本历史，相关源码和脚本已移除。

```sh
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-155
```

2026-10-31：固件对齐后 Loop 0.3.0 的 workspace 15 项测试通过，另有 1 项按需布局导出；实际设备 UI 的 Band 11 `.139/.155/.179` 分支各自通过主机测试。包含 Band 10 Pro 滚动条模式仅修改偏移 60 一个字节、Band 11 不写入该结构的检查。此前插件 5 项主机测试通过。`.036/.043/.139/.155/.179` 五个模块 ELF、完整包通过校验，均为零未定义导入，生成 Lua 语法检查通过。插件 Wasm 组件此前通过 wasmparser 校验，Connect 导入/导出存在；`.abp` 的 manifest、权限及 Wasm 载荷一致。UI 分层、布局预览及复现命令见 [UI-REWRITE.md](UI-REWRITE.md)。
尚未做真机安装、Launcher、页面生命周期和课表导入回归。

当前上游 Canopus `.155` Supervisor 使用默认 clang 18 时超出文件预算；clang 19 也仍超出。Loop 的 `.155` 模块构建通过不代表相应 Supervisor 已构建成功。详见 [ABI 2 迁移记录](MIGRATION-ABI2.md)，勿使用失败构建的 Supervisor 产物部署。
