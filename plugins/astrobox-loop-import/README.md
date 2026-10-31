# AstroBox Loop Import

AstroBox NG API Level 4 插件（WASI Preview 3 运行时）：选择 ICS 导出软件来源（支持 WakeUp / WeekDown / Nexio），
导入本地 `.ics` 并在插件内转换，经 CanopusConnect 直接推送到原生 Loop 模块。

协议见仓库 `docs/SCHEDULE_IMPORT_PROTOCOL.md`，共享实现在 `crates/loop-protocol`。启用手表 Loop 后即可导入，无需额外快应用。当前 Connect 服务目标为 `.043` / `.155` / `.179`；`.036` / `.139` 不提供此服务。

## 构建

```sh
# 在 WSL Ubuntu 中执行
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
rustup target add wasm32-wasip2
cargo test --target x86_64-unknown-linux-gnu
python3 scripts/build_dist.py --release --package
```

输出位于 `dist/`（含 `astrobox_plugin_loop_import.wasm` 和 `Loop课表导入器.abp`）。插件 0.3.0 采用逐片确认、CRC32 校验、超时重试，收到设备原子写入完成的确认后才显示成功。权限为 `canopusconnect.send`、`canopusconnect.listen` 和 `device`。

构建目标沿用 `wasm32-wasip2`，宿主的 Level 4 运行时兼容其标准库接口；
插件 API 使用 `psys-world-v4-canopusconnect` 的原生异步接口，需要支持 API Level 4 和 CanopusConnect 的 AstroBox。
WIT 来源和同步版本见 `wit/UPSTREAM.md`。
