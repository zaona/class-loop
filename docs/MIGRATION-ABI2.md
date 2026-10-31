# Canopus / Loop 新架构迁移记录

当前为 0.3.4 / versionCode 7：v1 文案、按钮和原生页面栈恢复，保留真实日期修复，见 [V1-PARITY-034.md](V1-PARITY-034.md)。下文旧版 UI 与构建结果保留为历史。

2026-10-31。按用户要求，所有新版编译、测试和打包均在 WSL 完成。

环境建立经过、可复现命令、开发循环和排错已整理在 [WSL 开发与构建流程](WSL-DEVELOPMENT.md)；当前结构与 Lyra `ng` 的逐项对照见 [LYRA-NG-COMPARISON.md](LYRA-NG-COMPARISON.md)。

此前诊断轮按用户要求改用 Canopus `ds-newdevice-supports@7995f192`，生成 Loop 0.3.2 / versionCode 5 启动诊断包 `dist/loop-0.3.2.canopus`。诊断版复现方法见 [第二轮设备排查](DEVICE-CRASH-29598-2.md)，新分支上游检查限制见 [新分支适配记录](DS-NEWDEVICE-SUPPORTS.md)；下文的 main/0.2.0/0.3.0 为迁移历史。

## 仓库

- Canopus：`main` 更新到 `4928f1eb`。
- LyraPlayer：`main` 更新到 `8b228e3` 后发现仍为旧架构，切到跟踪 `origin/ng` 的 `ng`，版本 `4dbb2eb`；未修改业务代码。
- Loop：基于 `master` 的 `6719914` 完成 ABI 2 适配，版本升为 `0.2.0` / `versionCode = 2`，改动尚未提交。

没有提交、推送或发布。没有修改固件地址批准状态或扩大校验范围。

## WSL 环境和入口

WSL 3.0.3 / WSL2，发行版 `Ubuntu-24.04`（24.04.5 LTS），当前用 root；VHD 位于 `D:\Canopus\.wsl\Ubuntu-24.04/`。
Linux Rust 1.99.1、`thumbv8m.main-none-eabi`、clang/lld/llvm 18、clang/lld 19、Python 3.12 和 Lua 5.4 已安装。

PowerShell 进入发行版：

```powershell
wsl -d Ubuntu-24.04 -u root
```

以下命令在 WSL 内执行：

```sh
. "$HOME/.cargo/env"
export CANOPUS_ROOT=/mnt/d/Canopus/Canopus
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
export PATH="$CARGO_TARGET_DIR/debug:$PATH"
cd /mnt/d/Canopus/Canopus
cargo build -p canopus-cli
canopus doctor

cd /mnt/d/Canopus/class-loop
sh scripts/build-wsl.sh
```

主机 Cargo 产物在 `/root/.cache/canopus/host`。官方 Supervisor 脚本查找源码树下的 `target/debug/canopus`，需从该缓存复制 CLI 或单独以默认 target 目录构建。
仓库本地 `core.autocrlf=input`；WSL 执行的 `.sh` 和 generated `.h` 保持 LF。不要用旧的 Windows overlays 或私钥流程。

## ABI 2 阶段已验证结果（0.2.0 历史记录）

| 项目 | 结果 |
|---|---|
| Canopus CLI / doctor | 构建成功，自检通过 |
| SDK abi / ui-core / runtime | 主机测试通过 |
| Supervisor `.036` | 125672 文件字节 / 160152 resident，通过 |
| Supervisor `.043` | 132824 文件字节 / 171536 resident，通过 |
| Supervisor `.139` | 133936 文件字节，通过 |
| Supervisor `.155` | 未通过文件预算，见下文 |
| Lyra `ng` | 88 项主机测试通过；`.036/.043/.139/.155/.179` 五目标构建和包校验通过 |
| Loop | 6 项主机测试通过；四目标 ELF、全量包及单 `.155` 包校验通过；安装器 Lua 语法检查通过 |

Loop resident 大小：`.036` 145454、`.043` 145470、`.139/.155` 145538 字节，全部零未定义导入。

### `.155` Supervisor 的上游限制

未改动的官方构建脚本，在 clang 18 下产物 140676 字节；clang 19 下 136824 字节，均超过 135168 字节预算。保留原预算及失败状态；这些失败构建产物不能作为验证通过的安装资源。
Loop / Lyra 的 `.155` 模块通过，不代表该 Supervisor 已通过。未做真机安装验证。

重现命令：

```sh
cd /mnt/d/Canopus/Canopus
CANOPUS_TARGET=xiaomi-band-11-4.100.155 sh scripts/build_canopus_supervisor.sh
CANOPUS_TARGET=xiaomi-band-11-4.100.155 CC=clang-19 sh scripts/build_canopus_supervisor.sh
```

## Loop 迁移内容

- 清单接入官方 Rust build planner、ABI 2、精确四目标和递增版本；使用 SDK 共享 ctor / module linker。
- 保存 Supervisor context，文件 / 内存 / 时钟改用服务表，激活前检查所需服务。
- Launcher 服务负责动态应用 ID 和分阶段发布，移除模块直接注册应用及固定 ID；五页仍使用精确固件绑定。
- 修正 Band 10 Pro 暂停 / UI 销毁回调槽位，返回时使用固件传入的 live page 指针。
- 图标嵌入 ELF 并在注册前写入模块私有目录；0.2.0 当时保留了快应用课表导入路径和 Band 11 回退，现已由下节 Connect 迁移替代。
- 删除旧 CMI1 签名、opaque-word 改写及旧安装器 Lua 源流程，统一官方 CMI2 打包与设备族安装器。
- 常量例外逐个对应图标像素、Rust Unicode 表及数字转换数据；未增加固件地址范围例外。
- WSL 入口为 `class-loop/scripts/build-wsl.sh`，在仓库内准备环境并调用 `scripts/build.py`；部分目标产物在 `dist/selected/<版本组合>/`，完整产物在 `dist/`。

## CanopusConnect 迁移（0.3.0）

按用户要求移除额外的手表导入快应用，Loop 和 AstroBox 插件版本更新为 0.3.0，模块 versionCode 为 3。新增 `loop-protocol`，workspace 共三个 crate；手机和设备共用 LPC1 二进制分片协议。每帧 480 字节、课表上限 40960 字节，逐片确认、CRC32 校验、5 秒超时与最多三次重试；设备 60 秒清理未完成传输。

原生模块直接监听 `loop` 通道，完整校验后写临时文件、sync、close、rename，再确认导入成功。新课表存于 `/data/canopus/m.loop/schedule.json`，界面在下一 tick 重载。删除 `quickapps/loop-import` 和手机旧 interconnect 实现，旧沙箱数据需重新推送。当前框架 `.043` / `.155` 提供 Connect；`.036` / `.139` 不提供，但 Loop 仍能离线启动。

WSL workspace 测试 9 项通过（core 4、device 3、protocol 2），包含实际保存函数的短写、写入/同步/关闭/rename 失败保护，以及共享协议到私有存储的完整链路。四目标 ELF 校验及完整 0.3.0 包、安装器 Lua 校验通过，零未定义导入。resident 字节数分别为 `.036` 148494、`.043` 148510、`.139/.155` 148562。新增常量例外均已对照 ELF 符号确认为 Rust Unicode 压缩表数据；保持固件地址批准范围不变。

插件采用 API Level 4 的 `psys-world-v4-canopusconnect` 与 wit-bindgen 0.63 异步绑定，权限改为 Connect send/listen 和 device。插件 5 项 ICS 测试通过，WSL 成功构建 Wasm 和 `.abp`；wasmparser 组件校验通过，确认 Connect 导入/导出，包内 manifest、权限、Wasm 载荷一致。插件及设备的最终验证日志保存于 `build/migration-abi2/connect-*.log`。蓝牙传输、灭屏导入和实际文件系统断电保护仍需真机验证。

## Lyra 式 UI 完全重写（0.3.0 未发布版本）

按用户后续要求，移除旧 `Tree/Snapshot` 和 `ui_backend.rs`，core/device 不再依赖 `canopus-ui-core`。改为 Lyra ng 的 `core/view + core/layout + device/target/ui` 分层，直接组合 LVGL 卡片、文本、滚动容器、进度条和动画。保留课表五页、动态 Launcher ID 和 CanopusConnect；详情长文本循环滚动，长课表按 tick 分批创建。

WSL workspace 14 项测试通过（core 8、device 3、protocol 2、实际设备 UI 综合测试 1），Band 11 UI 分支另行通过；1 项默认忽略的布局导出分别在两种面板运行成功。测试核对了 60 门课程、完整 u32 ID、导入后旧事件和清空确认失效、无变化不写文本、暂停/销毁与定时器释放。已查看从实际控件树绘制的两种面板五页预览，字体近似，仍非真机或 Mirage 结果。

四目标 ELF、完整包与安装器 Lua 校验通过，零未定义导入；resident 字节数为 `.036` 145162、`.043` 145130、`.139/.155` 144686。完整包 `dist/loop-0.3.0.canopus` 为 970752 字节，versionCode 3。新增精确常量 `0x0c010000` 已追踪至 `parse_ymd` 月份范围的 Rust 数据及填充，未放行地址范围。Canopus 和 Lyra 源码保持原样。

源码对照、主机测试、布局导出和验证边界见 [UI-REWRITE.md](UI-REWRITE.md)。日志为 `build/migration-abi2/ui-rewrite-tests.log`、`ui-host-band11-tests.log`、`ui-rewrite-build.log`。

## 五固件与滚动处理对齐（0.3.0 未发布版本）

新增 Band 11 `.179`，与 Lyra 的 `.036/.043/.139/.155/.179` 五目标清单一致。同步 Cargo feature、精确 SDK 绑定、Band 11 面板/事件和页面生命周期判断；动态 Launcher ID 与 Band 11 page key 返回方式保持一致。

滚动容器对齐 Lyra 的 `quiet_scroll_bar`：Band 10 Pro stock page-content 仅将 +60 的私有模式字节置 0；Band 11 三个目标不写该结构。仅修改 Loop，没有调整上游符号批准状态、预算或 verifier 例外。具体依据和主机字节边界检查见 [UI-REWRITE.md](UI-REWRITE.md)。

WSL workspace 15 项测试通过；Band 11 三个 UI 分支分别通过 2 项测试。五目标官方 ELF verifier、完整包、安装器 Lua 校验通过，零未定义导入。完整包为 1212416 字节，仍为未发布的 0.3.0 / versionCode 3。

| 目标 | resident 字节 | ELF SHA-256 |
|---|---:|---|
| `.036` | 145162 | `b3ef08a3851b6e1605417fed83216838a57a9882fef3979438152d6206dca874` |
| `.043` | 145130 | `2589c53aaa5e033deee41f984f637110ed3f7bf567639407a1e11721da74d8f6` |
| `.139` | 144686 | `3d7691705f51516ce68a0ac6111be7470e81c8f28078cb1c3d532346ec314677` |
| `.155` | 144686 | `97f9ca5e2a6ce12507f415327d574f8d89c01797d1fa0615faf73826fbf51f1d` |
| `.179` | 144734 | `a2a7a80b82a00d66f5a4557188d301778bb558f31194a6ee9dba9c34baa82b53` |

原始日志为 `build/migration-abi2/firmware-alignment-*.log`。Loop 的真机/Mirage 滚动与 `.179` 端到端导入尚未验证，未提升任何设备能力状态。

## 当前产物与后续设备检查

- Loop 完整包：仓库根目录下 `dist/loop-0.3.2.canopus`，构建来源在同名 `.build-info.json`。
- Loop 安装器：仓库根目录下 `dist/installer/xiaomi-band-10-pro/`、`xiaomi-band-11/`。
- Lyra 包：相邻仓库 `Canopus-App-LyraPlayer/dist/lyra_player.canopus`。
- 构建与测试日志：本仓库的 `build/migration-abi2/`（本地忽略）。

ABI major 已变化，需先重装匹配固件的当前 Supervisor。设备检查仍包括首次安装/重启、Launcher 动态 ID、页面切换/返回、灭屏恢复、销毁时定时器释放、Connect 导入/刷新/清空课表及升级结果。主机和静态验证不提升设备验证状态。
