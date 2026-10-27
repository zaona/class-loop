# AstroBox Loop Import

AstroBox NG API Level 4 插件（WASI Preview 3 运行时）：选择 ICS 导出软件来源（支持 WakeUp / WeekDown / Nexio），
导入本地 `.ics` 并在插件内转换，经 interconnect 推送到手表快应用
`top.zaona.loopimport`。

协议见仓库 `docs/SCHEDULE_IMPORT_PROTOCOL.md`。推送时请保持手表 Loop Import 前台打开。

## 构建

```sh
python scripts/build_dist.py --release --package
```

输出位于 `dist/`（含 `.abp`）。

构建目标沿用 `wasm32-wasip2`，宿主的 Level 4 运行时兼容其标准库接口；
插件 API 使用 `psys-world-v4` 的原生异步接口，需要支持 API Level 4 的 AstroBox。
WIT 来源和同步版本见 `wit/UPSTREAM.md`。
