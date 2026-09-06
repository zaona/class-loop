# Loop Import Protocol v1

本文定义 AstroBox `astrobox-loop-import` 插件与 Vela 快应用 `top.zaona.loopimport`
之间的课表推送协议。快应用工程位于 `quickapps/loop-import`。Loop 原生模块不参与
该协议，只读最终落盘的 `schedule.json`。

## 1. 架构

```text
课表.ics / schedule.json
          │
          ▼
AstroBox Loop Import 插件（.ics 在插件内转换）
          │  interconnect: top.zaona.loopimport
          ▼
Vela Loop Import 快应用
          │  internal://files/loop/schedule.json
          ▼
/data/files/top.zaona.loopimport/loop/schedule.json
          │  只读
          ▼
Loop 原生课表应用
```

快应用必须保持前台运行以便接收消息。插件只向 `top.zaona.loopimport` 收发。
ICS 转换语义与 `scripts/ics-to-schedule.py` 对齐（WEEKLY RRULE / INTERVAL / UNTIL / COUNT / BYDAY / EXDATE）。
主机脚本可用于离线校验；Loop 固件不内置课表样例。

## 2. 传输约束

- 协议版本：`1`
- 消息编码：UTF-8 JSON（单帧，无 Base64 分片）
- AstroBox → 快应用发送帧上限：49152 字节
- 快应用 → AstroBox 接收帧上限：8192 字节
- `schedule` 对象序列化后建议 ≤ 40960 字节（`maxScheduleBytes`）
- 课表 schema：见 `docs/SCHEDULE.md`（`version: 1`）

## 3. 握手

插件发送：

```json
{"tag":"loop-import-hello","version":1}
```

快应用回复：

```json
{
  "tag":"loop-import-hello",
  "version":1,
  "ok":true,
  "maxScheduleBytes":40960
}
```

## 4. 发布

插件发送：

```json
{
  "tag":"loop-import-publish",
  "version":1,
  "id":"transaction-id",
  "schedule":{
    "version":1,
    "term":{"name":"2026秋","start_date":"2026-08-31"},
    "courses":[]
  }
}
```

快应用校验 `schedule.version === 1` 与 `term` / `courses` 后，原子写入
`internal://files/loop/schedule.json`（先写 `.tmp` 再 move），然后回复：

```json
{
  "tag":"loop-import-ack",
  "version":1,
  "id":"transaction-id",
  "courses":42
}
```

失败时：

```json
{
  "tag":"loop-import-error",
  "version":1,
  "id":"transaction-id",
  "code":"publish-failed",
  "message":"reason"
}
```

## 5. 落盘规则

- 正式路径：`internal://files/loop/schedule.json`
- 临时路径：`internal://files/loop/schedule.json.tmp`
- 原生模块只读正式路径；半成品 tmp 不被读取
