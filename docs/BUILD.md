# Loop 构建说明（Windows）

## 前置条件

- 目录布局：`D:\Canopus\Canopus`、`D:\Canopus\class-loop`、`D:\Canopus\Canopus-Build`
- Rust nightly + `thumbv8m.main-none-eabi` + `llvm-tools`
- Android NDK 28（提供 `clang` / `ld.lld`）
- Git Bash、Python 3、`luac`（可选语法检查）
- 已构建 Canopus CLI：`cargo build --manifest-path D:\Canopus\Canopus\Cargo.toml`
- 签名私钥：`D:\Canopus\Canopus-Build\keys\module-installer-ed25519.pem`

## 环境变量

在 **Git Bash** 中：

```sh
. /d/Canopus/Canopus-Build/env.sh
```

`env.sh` 会设置 `PYTHONUTF8=1`、NDK PATH、`RUST_OBJCOPY`、`NM`、`MODULE_INSTALL_KEY`。

也可使用包装脚本：

```sh
/d/Canopus/Canopus-Build/build-loop.sh
/d/Canopus/Canopus-Build/build-loop-prod.sh
```

## 单 target

默认 `xiaomi-band-10-pro-3.101.036`：

```sh
/d/Canopus/class-loop/scripts/build-install-watchface.sh
```

指定 043：

```sh
CANOPUS_TARGET=xiaomi-band-10-pro-3.101.043 \
  /d/Canopus/class-loop/scripts/build-install-watchface.sh
```

产物：

- `build/<target>/loop.elf` — 已校验的 ET_REL 模块
- `build/<target>/receipt.bin` — CMI1 收据
- `watchfaces/loop/module.bin` + `receipt.bin` — 安装表盘载荷

## 双 target 生产包

```sh
/d/Canopus/class-loop/scripts/build-install-watchface-prod.sh
```

产出 `watchfaces/loop-prod/loop-<target>.bin` 与 `.cmi.bin`，安装器按 `ro.build.version` 选择 payload。

## 主机测试

```sh
cargo test -p loop-core
cargo test -p loop-device --no-default-features
```

## 注意

- Loop 的 ctor 已内置 `.rodata.cst8` 锚点，**不依赖** Lyra 的 Windows overlay。
- 私钥勿提交进 git。
