# Loop Import 快应用

包名：`top.zaona.loopimport`

负责把课表 JSON 写进 `internal://files/loop/schedule.json`，供 Loop 原生只读。
手机侧由 AstroBox 插件推送（可直接选 `.ics`）。协议与 schema 见：

- [docs/SCHEDULE.md](../../docs/SCHEDULE.md)
- [docs/SCHEDULE_IMPORT_PROTOCOL.md](../../docs/SCHEDULE_IMPORT_PROTOCOL.md)

> **请勿在导入课后删除本快应用。** 卸载会清空已发布的 `schedule.json`。

物理路径：`/data/files/top.zaona.loopimport/loop/schedule.json`。

保持本页前台打开，由 AstroBox 插件推送课表。Loop 无内置课表：未推送时腕上显示「尚未导入课表」。

## 构建

```sh
cd quickapps/loop-import
npm install
npm run build
```
