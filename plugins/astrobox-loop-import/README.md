# AstroBox Loop Import

AstroBox NG API level 3 插件：选择 ICS 导出软件来源（支持 WakeUp / WeekDown / Nexio），
导入本地 `.ics` 并在插件内转换，经 interconnect 推送到手表快应用
`top.zaona.loopimport`。

协议见仓库 `docs/SCHEDULE_IMPORT_PROTOCOL.md`。推送时请保持手表 Loop Import 前台打开。

## 构建

```sh
python scripts/build_dist.py --release --package
```

输出位于 `dist/`（含 `.abp`）。
