# WSL 开发与构建流程

记录日期：2026-10-31，更新至 2026-11-01。适用于当前 `D:\Canopus` 工作区。本文件记录环境搭建、日常修改、构建验证和排错；[BUILD.md](BUILD.md) 是日常构建速查，[MIGRATION-ABI2.md](MIGRATION-ABI2.md) 保存本次迁移结果。

此前诊断轮已切换 Canopus `ds-newdevice-supports@7995f192`，生成 Loop 0.3.2 / versionCode 5 启动诊断版。新分支 WSL 命令和上游检查限制见 [DS-NEWDEVICE-SUPPORTS.md](DS-NEWDEVICE-SUPPORTS.md)；2026-11-01 的设备日志对比、诊断实现、验证命令和包哈希见 [DEVICE-CRASH-29598-2.md](DEVICE-CRASH-29598-2.md)。`build-wsl.sh` 默认先编译同一 checkout 的 CLI，再构建模块，并生成 `.build-info.json`。

2026-11-01 第三份真机日志已通过 WSL 解压、解析并与上一份逐字节对比，确认 0.3.2 首页创建成功且没有新增同类 BusFault；512 条诊断记录因限额停止。此轮没有改运行时代码或重建，详细证据与结论边界见 [DEVICE-CRASH-29598-3.md](DEVICE-CRASH-29598-3.md)。

当前版本为 0.3.5 / versionCode 8，恢复 v1 文案、全部按钮和五个原生页面，使用管理器同类系统预制组件，恢复固件切页和 UI 线程刷新，不需要替换现有管理器。逐项对照、WSL 命令与验证结果见 [V1-PARITY-034.md](V1-PARITY-034.md)。0.3.3 的系统组件迁移、第 0 周修复、Corona 克隆及网络处理保留为历史，见 [CORONA-UI-033.md](CORONA-UI-033.md)；真实日历修复继续保留。

0.3.5 在 WSL 中扩展原生交互回归，修复按住课程期间导入的旧点击和前进/返回失败后的恢复。已删除两个旧表盘构建入口、重复 ELF 校验脚本、四份 `targets/*.env`、`index.txt` 和重复图标。命令、清理范围及结果见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)，日志为 `build/review-fixes-035/`。后文迁移时期的旧测试和版本号保留为历史。

提交前还处理了编辑器显示的 12 条存储模块未使用警告：主机测试仅引入部分设备存储调用路径，因此只在 `storage_host` 测试模块上豁免 `dead_code`；正式设备编译仍保留该检查。此调整不改变设备运行时代码。

## 1. 当前环境与目录

| 项目 | 本次实际值 |
|---|---|
| Windows 工作区 | `D:\Canopus` |
| Linux 工作区 | `/mnt/d/Canopus`，与 Windows 共享同一份源码 |
| WSL / 内核 | WSL 3.0.3.0 / 6.18.54.1-1，WSL2 |
| 发行版 / 当前用户 | `Ubuntu-24.04`，Ubuntu 24.04.5 LTS / root |
| 发行版磁盘位置 | `D:\Canopus\.wsl\Ubuntu-24.04` |
| Rust / Cargo | 1.99.1 / 1.99.1，Linux 版本 |
| Rust target | `x86_64-unknown-linux-gnu`、`thumbv8m.main-none-eabi`、`wasm32-wasip2` |
| clang / lld | 默认 18.1.3；另装 clang 19.1.1 / lld 19 做 Supervisor 对照构建 |
| Python / Lua | Python 3.12.3 / Lua 5.4.6，均为 Linux 版本 |
| 主机 Cargo 缓存目录 | `/root/.cache/canopus/host` |
| CLI 主机产物 | `/root/.cache/canopus/host/debug/canopus` |

```text
D:\Canopus\                       /mnt/d/Canopus/
  .wsl/                           WSL 发行版磁盘，独立于源码仓库
  Canopus/                        框架、SDK、target packs、官方 CLI
  Canopus-App-LyraPlayer/          ng 分支参考应用
  Canopus-Module-Corona/            系统组件 UI 参考，源码保持原样
  Canopus-Module-Resource-Hook/     其他模块
  class-loop/                     Loop 源码、构建入口和本文
```

使用 Windows 编辑器修改文件，编译和测试命令在 WSL 中执行。不需要将源码重复复制到 Linux；Rust 与 Python 等工具使用 Linux 安装，不调用 Windows 的 `cargo.exe` 或 `python.exe`。

## 2. 本次环境建立经过

1. 最初只有 Windows 的 WSL 入口，runtime 与发行版尚不可用。启动安装后用户完成确认并重启。
2. 重启后系统仍提示需要 `wsl --update`。完成 WSL runtime 安装和发行版初始化，最终确认 `Ubuntu-24.04` 能以 WSL2 运行。
3. 安装 Linux 侧编译依赖、Rust 1.99.1、ARM target、Python 和 Lua；默认使用 LLVM 18，额外安装 LLVM 19 做 `.155` Supervisor 体积比较。
4. Rust 官方下载较慢，安装时临时使用 `rsproxy.cn`；没有将镜像变量作为日常构建依赖。曾复用公开 Cargo registry 下载缓存，依赖声明和锁文件仍由 Cargo 管理。
5. 处理 Windows checkout 带来的 CRLF：仓库本地设为 `core.autocrlf=input`，运行的 shell 脚本及 Supervisor 使用的生成头文件转成 LF。
6. 在 WSL 构建 CLI、自检和 SDK 测试，然后验证 Supervisor、Lyra `ng`，最后迁移 Loop。详细目标结果及失败项见迁移记录。
7. 最初的外部构建工具目录已按用户要求删除；必要日志迁入 `class-loop/build/migration-abi2/`，环境准备入口迁入 `scripts/build-wsl.sh`。删除后用清空相关环境变量的方式重跑四目标构建、打包和校验，全部通过。测试用 Snap 仓库也已按用户要求删除。

## 3. 在新机器上准备 WSL

以下为新环境的复现步骤，现有环境无需重复安装。Windows 命令在 PowerShell 中运行；启用系统组件时按系统要求提权和重启。

```powershell
wsl --update
wsl --status
wsl --list --verbose
```

尚无发行版时，可用当前 WSL 的安装位置选项：

```powershell
wsl --install -d Ubuntu-24.04 --location D:\Canopus\.wsl\Ubuntu-24.04 --no-launch
```

按照安装提示完成初始化。当前机器用 root；新机器可以使用自行建立的 Linux 用户，后续 `$HOME` 和缓存位置随该用户变化。当前工作区的进入命令是：

```powershell
wsl -d Ubuntu-24.04 -u root
```

在 Ubuntu 内安装依赖：

```sh
sudo apt-get update
sudo apt-get install build-essential git curl ca-certificates clang lld llvm \
  python3 lua5.4 pkg-config libssl-dev
```

通过 Rust 官方 rustup 安装 Linux Rust（已安装可跳过前两行）：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/canopus-rustup.sh
sh /tmp/canopus-rustup.sh -y --profile minimal --default-toolchain stable
. "$HOME/.cargo/env"
rustup component add rustfmt
rustup target add thumbv8m.main-none-eabi wasm32-wasip2
```

精确复现本次 Rust 版本时，可安装 `1.99.1` 并在当前 shell 设置 `RUSTUP_TOOLCHAIN`，不必改机器全局默认值：

```sh
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --target thumbv8m.main-none-eabi
export RUSTUP_TOOLCHAIN=1.99.1
```

确认工具来自 Linux：

```sh
command -v cargo clang ld.lld python3 luac
rustc --version
rustup target list --installed
clang --version
python3 --version
luac -v
```

## 4. 每次开发时的环境

Loop 完整构建可直接运行下面的仓库入口，它会自动加载 Cargo 环境并准备路径：

```sh
cd /mnt/d/Canopus/class-loop
sh scripts/build-wsl.sh
```

需要分别操作框架、Lyra、测试或 CLI 时，在当前 WSL shell 设置：

```sh
. "$HOME/.cargo/env"
export CANOPUS_ROOT=/mnt/d/Canopus/Canopus
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
export PATH="$CARGO_TARGET_DIR/debug:$PATH"
export PYTHONUTF8=1
```

`build-wsl.sh` 是子进程，退出后不会给父 shell 留下这些环境变量。脚本默认推导相邻的 Canopus 仓库，支持 `CANOPUS_ROOT`、`CARGO_TARGET_DIR` 和 `CANOPUS_CLI` 覆盖。更换主机缓存路径后，须在对应路径构建 CLI，或通过 `CANOPUS_CLI` 指定已有二进制。

## 5. 更新源码与构建框架

先检查各仓库的 `git status --short` 和当前分支，保存自己的修改后再拉取；使用 `git pull --ff-only` 保持更新可追踪。不要对含未提交修改的目录执行清理或覆盖。当前核对版本是 Canopus `ds-newdevice-supports@7995f192`、Lyra `ng@4dbb2eb`；Lyra 的 `main` 与 `ng` 属于不同架构阶段。Canopus 原 `main@4928f1eb` 仅作为历史迁移基线。

准备好上一节的环境后：

```sh
cd "$CANOPUS_ROOT"
cargo build -p canopus-cli
canopus doctor
cargo test -p canopus-abi -p canopus-ui-core -p canopus-runtime
```

官方 Supervisor 构建脚本固定查找源码树的 `target/debug/canopus`。使用 Linux 主机缓存时，构建后同步 CLI：

```sh
mkdir -p "$CANOPUS_ROOT/target/debug"
cp "$CARGO_TARGET_DIR/debug/canopus" "$CANOPUS_ROOT/target/debug/canopus"
CANOPUS_TARGET=xiaomi-band-10-pro-3.101.036 sh scripts/build_canopus_supervisor.sh
```

替换 `CANOPUS_TARGET` 可分别构建 `.043`、`.139` 和 `.155`。本次前三个通过；`.155` 在 clang 18/19 下仍超出文件预算，不能将其失败产物用于部署。原定预算和固件符号批准状态均保持不变。

## 6. 验证参考应用 Lyra ng

在同一 WSL 环境中：

```sh
cd /mnt/d/Canopus/Canopus-App-LyraPlayer
git branch --show-current
cargo test --workspace
canopus build
canopus package --output dist/lyra_player.canopus
canopus package verify dist/lyra_player.canopus
```

当前 `ng` 的 README 采用官方 CLI 路线。同仓库残留的旧 `build-install-watchface*.sh` 仍含 CMI1/签名步骤，本次没有用它们生成新版包，也没有修改 Lyra 业务代码。目录及运行时差异见 [Lyra ng 对照](LYRA-NG-COMPARISON.md)。

## 7. 修改、测试和构建 Loop

编辑课表逻辑通常在 `crates/loop-core/`；固件接口、模块生命周期与界面适配在 `crates/loop-device/`；共享协议在 `crates/loop-protocol/`，手机插件在 `plugins/astrobox-loop-import/`；额外的手表导入快应用已移除。

```sh
cd /mnt/d/Canopus/class-loop
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
sh scripts/build-wsl.sh
```

需要修改格式时，对同一组 package 运行 `cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host`。避免对这类含本地路径依赖的工程直接执行 `cargo fmt --all`，它可能连带修改相邻 SDK；本次曾逐文件确认并备份、还原此类格式差异。

UI 自 0.3.3 改用管理器 / Corona 的系统组件。展示规则在 `loop-core/src/view.rs`，组件和事件在 `loop-device/src/target/ui/mod.rs`。`tools/loop-ui-host` 直接编译该设备源码，使用 SDK 语义树验证长列表、导入、完整详情、清空确认与返回。固件渲染、字体和滚动由框架处理；不再有自制像素布局、Band 11 UI feature 分支或绘图预览脚本。主机测试不能替代真机绘制验收。

只迭代某个设备或固件时：

```sh
sh scripts/build-wsl.sh xiaomi-band-11
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
```

入口依次执行交叉编译、官方 ELF verifier、`.canopus` 打包、安装器生成、包校验和 `luac -p`。部分目标会在项目根目录生成临时清单，构建结束后删除；全量和部分安装器分目录保存。

| 产物 | 路径（相对 Loop 根目录） |
|---|---|
| 精确固件模块 | `build/<target>/loop.elf` |
| 五目标包 | `dist/loop-0.3.3.canopus`，旁边有 `.build-info.json` |
| 完整设备族安装器 | `dist/installer/<family>/` |
| 部分目标包与安装器 | `dist/selected/<版本组合>/` 及其 `installer/` |
| 本次保留的原始构建与测试日志 | `build/migration-abi2/`，已被 Git 忽略 |

主流程命令不包含 AstroBox 插件的独立打包；见下节。导入协议见 [SCHEDULE_IMPORT_PROTOCOL.md](SCHEDULE_IMPORT_PROTOCOL.md)。

2026-10-31 进一步核对了旧文件：图标及可复现的 PNG 源已归入 `crates/loop-device/assets/`，历史提醒文档已标注旧基线。旧目录、脚本和安装包的具体待删清单见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)，批量删除尚待确认。此次资源调整的验证日志为 `build/migration-abi2/cleanup-tests.log` 和 `cleanup-build.log`。

保存一次新的完整构建日志并保留退出状态，可在 WSL 中这样执行：

```sh
mkdir -p build/logs
sh scripts/build-wsl.sh > build/logs/loop-wsl.log 2>&1
result=$?
tail -n 40 build/logs/loop-wsl.log
test "$result" -eq 0
```

## 8. AstroBox 插件开发和打包

Loop 0.3.0 已改为 CanopusConnect 直接导入，手机与设备共享 `loop-protocol`；旧 `quickapps/loop-import` 已完整移除。当前 `.043` / `.155` / `.179` 提供 Connect，`.036` / `.139` 只能离线使用。原快应用中的课表需从手机重新推送。

在 WSL 中准备第 4 节环境，然后执行：

```sh
cd /mnt/d/Canopus/class-loop/plugins/astrobox-loop-import
rustup target add wasm32-wasip2
cargo fmt -- --check
cargo test --target x86_64-unknown-linux-gnu
python3 scripts/build_dist.py --release --package
```

插件是独立 Cargo workspace，配置默认 target 为 `wasm32-wasip2`；主机测试需要显式指定 Linux target。WIT world 为 `psys-world-v4-canopusconnect`，采用 wit-bindgen 0.63 的异步绑定。产物为插件 `dist/astrobox_plugin_loop_import.wasm` 和 `dist/Loop课表导入器.abp`。

安装 `.abp` 后，在已连接设备列表选择设备，确认原生 Loop 已启用，再选择来源和 ICS 文件推送。无需额外导入快应用。插件等待设备完成校验与原子保存后的确认；逐片传输、CRC32、重试和私有存储规则见协议文档。

本次日志放在 Loop `build/migration-abi2/connect-tests.log`、`connect-build.log`、`connect-plugin-tests.log`、`connect-plugin-build.log`。测试包含重复分片、重复提交、损坏校验和，以及文件写入、同步、关闭、重命名失败时保留旧课表。

## 9. 校验、安装与问题定位

```sh
canopus package verify dist/loop-0.3.5.canopus
luac -p dist/installer/xiaomi-band-10-pro/main.lua
luac -p dist/installer/xiaomi-band-11/main.lua
```

本次还逐项检查过 CMI2：176 字节、模块 `loop`、versionCode 3、resident 生命周期、精确 target 和固件 SHA-256、ELF 大小与 SHA-256，以及安装器载荷与包内文件的一致性。当前五个 ELF 均为零未定义导入。

后续按用户要求对齐 Lyra 的五个固件目标：新增 `.179` Cargo feature、精确绑定及 Band 11 UI/生命周期分支。滚动容器接入 Lyra 的 `quiet_scroll_bar`，只对 Band 10 Pro stock page-content 的模式字节（偏移 60）写 0；Band 11 全部分支跳过结构写入。主机记录器改为持有真实的字节缓冲区，以执行并检查同一份设备代码。WSL workspace 15 项测试、三个 Band 11 UI 分支测试、五目标构建/打包和 Lua 语法检查通过。日志为 `build/migration-abi2/firmware-alignment-*.log`；该结果仍不替代真机滚动和导入验证。

安装时先准备匹配精确固件的当前 ABI 2 Supervisor，再使用对应设备族生成的安装器。旧 ABI 1 Supervisor 和 CMI1 回执不适用；当前 CMI2 无签名私钥步骤。安装器写入 inbox 的 `.cmi.next` / `.ko.next`，升级返回 `REBOOT_REQUIRED(8)` 表示重启后生效。

| 现象 | 本次经验与处理 |
|---|---|
| WSL 提示 runtime 过旧或不可启动 | 在 PowerShell 检查 `wsl --update`、`--status`、`--list --verbose`；系统要求重启时先完成重启 |
| 每次启动提示 localhost 代理未镜像到 WSL | 本机 WSL NAT 模式下出现过；本次实际下载可用。若网络失败，在 Linux 内检查实际代理和连通性 |
| `sh` 出现带 `\r` 的命令或参数错误 | 检查 CRLF；Loop 的 `.gitattributes` 已要求 `.sh` 使用 LF |
| Supervisor 报平台配置不完整，但头文件看起来有宏 | 检查生成头文件的 CRLF；脚本的行尾匹配可能受影响 |
| CLI 找不到或选到 Windows 工具 | 检查 `command -v`、Linux cache PATH、`CANOPUS_CLI` 和源码树 `target/debug/canopus` |
| WSL 直连 GitHub:443 超时，但 Windows Git 可连接 | 可在 WSL 调用 `git.exe fetch/push` 使用宿主现有网络与凭据；本地提交使用已配置的作者身份，避免误用 WSL root 默认身份。Rust 检查、构建和打包继续使用 Linux 工具链。 |
| verifier 报 embedded absolute address | 对照 ELF 节/符号与原始资产，先确认其来源；图标像素、Unicode 表等只添加有证据的精确常量。不能放行整个地址区间或绕开验证器 |
| 部分构建后 package 报产物过期 | 临时清单须先于构建生成；保持使用同一清单构建和打包，当前 Python 入口已处理 |
| `.155` Supervisor 体积失败 | clang 18 为 140676，clang 19 为 136824，预算 135168 字节；记录失败，不扩大预算 |

本次尚未完成 Loop 真机回归。首次安装、动态 Launcher ID、页面切换/返回、灭屏恢复、销毁时定时器释放、课表导入/刷新/清空和升级重启均仍需设备验证。
