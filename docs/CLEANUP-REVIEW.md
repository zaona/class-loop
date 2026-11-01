# Loop 未提交代码审查与清理

更新日期：2026-11-01。当前修复版为 **0.3.5 / versionCode 8**，沿用 Canopus `ds-newdevice-supports@7995f192`。用户已授权执行审查提出的修复和清理。

## 交互修复

- 课程行从 PRESSED 保存行号和内容，到 CLICKED 核对最新课表；即使 100ms 定时器已将新课程绘制进相同行，也丢弃旧点击。PRESS_LOST、暂停和销毁清理按下状态，重复点击不会再次执行操作。
- 固件前进、标题返回失败时恢复路由、完整历史、所选星期、课程和数据页状态，并恢复原页面定时器。只保存导航字段，不复制或回滚课表及日历时钟。
- 主机回归覆盖按住期间导入并刷新、点击维护期间导入、取消/重复点击，以及首页、星期、课程前进失败和详情返回失败后的恢复。正常文案和操作延续 [v1 对照](V1-PARITY-034.md)。

## 已删除的遗留文件

| 文件 | 清理依据 |
|---|---|
| `scripts/build-install-watchface.sh`、`scripts/build-install-watchface-prod.sh` | 绕过 WSL Cargo 初始化和同 checkout CLI 编译；统一使用 `sh scripts/build-wsl.sh [设备族或固件]`。 |
| `scripts/verify-device.sh` | 当前构建已执行官方完整 verifier。 |
| `targets/*.env`（四份） | 新流程无引用；目标清单来自 Canopus.toml，编译参数来自 Cargo/SDK。 |
| `watchfaces/loop/appicon_loop.bin` | 与设备 crate 内图标逐字节一致，SHA-256 为 `03bffccb1cfef8186c3cd1dede1588f304ac3d395aa19e56e0993bee05f4ddf4`。 |

.gitignore 将旧 watchfaces/ 统一作为遗留产物目录忽略，移除过期 main.lua 例外。历史说明仍被跟踪；README、BUILD、WSL 流程和 Lyra 对照更新，ICS 转换明确由手机插件负责。

此前删除的旧导入快应用、本地 ctor、opaque/staging 脚本和旧 UI 实现不恢复。

## 保留范围

保留 crates/loop-device/assets/ 的 PNG、BIN 和编码说明，业务源码、手机插件及 vendored WIT、测试输入、当前构建脚本。根目录 `index.txt` 是插件市场发布定位文件，内容为 `plugins/astrobox-loop-import/dist`；此前误删，现已按原始字节恢复，不属于无用文件。build/ 的迁移、设备故障、审查原始失败复现和修复日志及 Cargo 缓存继续保留。设备日志和旧版迁移/UI 文档属于历史证据，上课提醒文档仍为历史研究。

## 安装产物整理

用户随后授权清理过多的构建包。已从 `dist/` 删除 13 个旧 `.canopus` 包及旧 build-info、`.155` 旧安装器，并删除 `watchfaces/` 的过期 BIN/回执/ZIP。共移除 43 个文件，释放 9637793 字节（约 9.2 MiB）。空的旧 `.155` 选择目录同时移除。

最初保留 0.3.5 完整包和 `.043` 专用包，随后按用户要求又删除专用包、专用 build-info 和专用安装器，共 5 个文件、478190 字节；空的 `dist/selected/` 同时移除。当前 `dist/` 仅保留五目标完整包、同名 build-info 和完整包对应安装器，后续日常构建使用不带目标参数的 `sh scripts/build-wsl.sh`。

手机导入插件产物保持原状。清理文件清单、SHA-256 和旧 build-info 内容存于 `build/package-cleanup-035/removed-artifacts.json`；专用产物清单为同目录 `removed-selected-artifacts.json`。故障调查日志和 build/ 下的历史证据不受影响。本文及其他历史文档提到的旧 dist 包路径为当时记录，旧包和专用包现已清理。

第一轮清理后，两个包的 WSL 官方校验及 6 份安装器 ELF 哈希均通过。删除专用产物后再次核对完整包 SHA-256 与 build-info 一致，完整安装器不受影响。

## WSL 复现

```sh
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cd /mnt/d/Canopus/class-loop
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-155
cargo test --manifest-path plugins/astrobox-loop-import/Cargo.toml --target x86_64-unknown-linux-gnu
sh scripts/build-wsl.sh
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
git diff --check
```

本轮日志目录为 `build/review-fixes-035/`：workspace 14 项、Band 11 UI 分支另 1 项、插件 5 项测试全部通过；fmt 和 git diff --check 通过。五个目标的官方 ELF verifier 均 PASS、零未定义导入，完整包和 .043 包及其 CMI2 安装器、Lua 语法检查通过。`.043` resident 为 136700 字节。日志为 `tests.log`、`tests-band11.log`、`plugin-tests.log`、`build-all.log`、`build-043.log` 和 `diff-check.log`。

提交前消除了编辑器中 12 条主机测试的未使用警告：仅对独立引入设备存储源码的 `storage_host` 测试模块豁免 dead_code，正式设备构建仍检查该警告。WSL 的 `RUSTFLAGS=-Dwarnings cargo test --workspace` 与 `cargo check --workspace --all-targets` 均通过，日志为 `tests-no-warnings.log` 和 `check-no-warnings.log`。此项不改变设备运行时。

两个包及全部 ELF 的实际 SHA-256 均与同名 build-info 一致，SDK dirty=false。Canopus、Lyra、Corona 和 Resource Hook 工作区仍干净；未修改固件符号、verifier 放行常量或构建预算。原始失败复现保留于 `build/review-uncommitted/`；旧测试副本用于记录审查过程，不是当前验收入口。

| 包 | 字节数 | SHA-256 |
|---|---:|---|
| .043 专用（已删除，历史哈希） | 233984 | `a9bb2f6fa5e7ad89d49b6571b7d063e39646605390fd12a98a8ce1eccf71f58c` |
| 五目标完整 | 1161728 | `8415835a82a2206d2f91fa4492505733ce687f63977de5b98057f5be586a5369` |

当前唯一模块包为 `dist/loop-0.3.5.canopus`，包含全部五个目标，也适用于当前 `.043` 设备。不需要更新管理器，课表存储路径与导入协议不变。真实固件动画、触摸派发和绘制仍需设备验收，主机测试不能替代真机验证。
