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

指定 Band 11 `.139`：

```sh
CANOPUS_TARGET=xiaomi-band-11-4.100.139 \
  /d/Canopus/class-loop/scripts/build-install-watchface.sh
```

## 按设备生产包

不指定设备时构建 10 Pro（036+043）与 Band 11（139+155）：

```sh
/d/Canopus/class-loop/scripts/build-install-watchface-prod.sh
/d/Canopus/class-loop/scripts/build-install-watchface-prod.sh xiaomi-band-11
```

产出 `watchfaces/loop-prod/<device>/`：该目录的 `main.lua` 与 `loop-<target>.bin` / `.cmi.bin`。
安装器按 `ro.build.version`（及 Band 11 的 `ro.build.id`）选择 payload。
Band 11 使用 `/canopus/install`，10 Pro 仍可走 `/dev/canopus`。

## 主机测试

```sh
cargo test -p loop-core
cargo test -p loop-device --no-default-features
```

## 注意

- Loop 的 ctor 已内置 `.rodata.cst8` 锚点，**不依赖** Lyra 的 Windows overlay。
- 私钥勿提交进 git。
