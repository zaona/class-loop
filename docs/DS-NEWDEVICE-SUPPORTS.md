# Canopus ds-newdevice-supports 适配

当前为 0.3.4 / versionCode 7：v1 文案、按钮和原生页面栈恢复，保留真实日期修复，见 [V1-PARITY-034.md](V1-PARITY-034.md)。下文旧版 UI 与构建结果保留为历史。

2026-10-31，按用户指定将相邻 Canopus 从 `main@4928f1eb` 切到跟踪远程的 `ds-newdevice-supports`，commit `7995f19240c20b95228fa2b607fce0c3196eca37`。Canopus 源码未修改。Lyra 保持 `ng@4dbb2eb`。

本文记录 0.3.1 的分支适配。2026-11-01 用户反馈 Loop 仍重启、Lyra 正常，随后生成 0.3.2 / versionCode 5 启动诊断版，见 [第二轮设备排查](DEVICE-CRASH-29598-2.md)；不将本次分支适配视为故障修复。

## Loop 变化

- 版本为 0.3.1 / versionCode 4，避免与设备上已经安装测试的 0.3.0 混淆；AstroBox 插件仍为 0.3.0，共享 LPC1 协议没有变化，不需要为模块的构建迁移重新安装插件。
- 继续使用 ABI 2 context 文件/内存/时间/Launcher/Connect 服务、Supervisor 动态 app id、五个原生页面和现有五个精确固件目标。当前新分支没有要求改写这些服务签名；未凭新增设备记录扩展 Loop 的设备清单。
- WSL 入口默认从当前 SDK checkout 重新编译 CLI，再调用该 CLI 构建。原入口会从 PATH 拿到之前在 main 上编译的缓存 CLI，从而继续沿用旧的 Rust 链接流程；现已消除这个入口问题。
- 新 CLI 使用 `ld.lld -r --gc-sections` 并保留模块 ABI 根；Loop 已采用此官方流程。对五个 ELF 额外核对了入口、模块描述符、四个页面生命周期、事件及刷新/动画定时器回调没有被裁剪。
- 每次成功打包后输出同名 `.build-info.json`，保存 SDK 分支/commit/dirty 状态以及 CLI、包、各 ELF 的 SHA-256。

本次未修改 UI 运行时代码。旧日志中的 BusFault 尚未定位到具体调用，新分支重新编译成功不能证明真机软重启已经修复。原分析与故障版本二进制保留见 [DEVICE-CRASH-29598.md](DEVICE-CRASH-29598.md)。课表路径仍为 `/data/canopus/m.loop/schedule.json`。

## WSL 操作

以下命令在 Ubuntu-24.04 中运行，源码仍是 Windows 共享目录 `/mnt/d/Canopus`。

```sh
cd /mnt/d/Canopus/Canopus
git status --short
git fetch origin ds-newdevice-supports
git switch --track origin/ds-newdevice-supports  # 本次首次建立本地分支

cd ../class-loop
sh scripts/build-wsl.sh

. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
for firmware in 139 155 179; do
    cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-$firmware
done
```

分支已建立后使用 `git switch ds-newdevice-supports`，更新使用 `git pull --ff-only`。`canopus sdk update` 默认跟踪 main，本项目需要保持指定分支，日常使用这里的相邻 checkout 构建入口。

## 构建与验证

WSL CLI 构建及 doctor 通过。Loop workspace 15 项测试通过、1 项按需布局导出跳过；Band 11 `.139/.155/.179` 分支各 2 项通过、1 项布局导出跳过。五个 ELF 均通过官方 verifier，零未定义导入。完整包与两个设备族安装器通过包校验和 Lua 语法检查。

| 目标 | resident 字节 | ELF SHA-256 |
|---|---:|---|
| 10 Pro .036 | 137332 | fcc0cc4d5ce6a151b9ab4dad4802b4f1b6edd040535211f9bf6ff3cae448df29 |
| 10 Pro .043 | 137300 | 1b4be135e9f3ec07604c2d1359cf60b7c39d56af86385be75f983e9c675c3b8d |
| Band 11 .139 | 136924 | e1f786f0c8938d0d2e93d64957b7838876282180765108e507b8a70b613ef792 |
| Band 11 .155 | 136924 | 7239ab206162f65f0001cc31849444ff2064a50788f90aeda2b69d0df228618a |
| Band 11 .179 | 136972 | 929cec9c8e5e1287cd806219da2bebff3deb6a1ac76e5a7f6f32373b7d36ff8c |

`.043` 比旧 main 构建减少 7,830 resident 字节；只说明静态装载体积变化，不代表测得了运行时空闲堆或解决了 BusFault。

## 上游检查限制与配套 Supervisor

执行 `cargo test -p canopus-core --test generated_stability`：16 通过，3 失败。失败为 `c_veneer_regenerates_identically`、`rust_bindings_regenerate_identically`、`additional_target_artifacts_regenerate_identically`，首先报告 `.036` 的类型声明输出顺序不同。

已逐条比对三个失败输出：全文行多重集合一致、13 个结构体各自的字段顺序/内容一致。上游 `tools/canopus-core/src/veneer.rs::load_records` 按 `read_dir` 顺序读入类型而未排序，是跨文件系统声明顺序差异的来源。本次保持 Canopus 原样，未为了使检查变绿而重写生成文件。其他目标是否还有后续生成物差异不能从这个提前失败的测试推断。

针对用户当前 Band 10 Pro `.043` 另外重新构建配套 Supervisor：

- clang 18：134488 文件字节，超过官方 133120 字节预算，构建失败。
- clang 19：131280 / 133120 文件字节，170228 resident 字节，零未定义导入，官方 verifier 通过；SHA-256 为 `a16a059613b4796537b90a29bdcbdcdade9d0662bca1ea60f6361df8525195b8`。
- 没有调整体积预算、符号批准状态或 verifier 地址范围；本轮未重建其他固件 Supervisor，不能将本结果延伸到其他版本。

```sh
cd /mnt/d/Canopus/Canopus
cp "$CARGO_TARGET_DIR/debug/canopus" target/debug/canopus
CANOPUS_TARGET=xiaomi-band-10-pro-3.101.043 CC=clang-19 \
    sh scripts/build_canopus_supervisor.sh
```

通过的文件暂存于 `Canopus/watchfaces/canopus-installer-prod/xiaomi-band-10-pro/canopus_supervisor-xiaomi-band-10-pro-3.101.043.bin`。该目录含其他历史固件资源，不能据此认为整个目录都已在新分支重建；本轮没有封装管理器安装表盘包。

## 产物

- Loop 完整包：`dist/loop-0.3.1.canopus`，1,149,952 字节，五目标，versionCode 4。
- 包 SHA-256：`592eea148799bba22f682f9891b25681fdaff0bf2a4253351589404a8fbe8428`。
- 构建来源：`dist/loop-0.3.1.build-info.json`。
- 安装器：`dist/installer/xiaomi-band-10-pro/`、`dist/installer/xiaomi-band-11/`。
- 本地日志：`build/ds-newdevice-supports/{cli-build,doctor,loop-build,tests,sdk-generated-tests,supervisor-043,supervisor-043-clang19}.log`；声明顺序核对为 `sdk-order-diff.txt`。

尚未进行新包真机复测，无真机或模拟器能力状态提升。需要检查 Loop 首次打开、切页/返回、灭屏恢复和 Connect 课表导入；若仍重启，继续针对旧分析缺失的启动阶段记录定位。
