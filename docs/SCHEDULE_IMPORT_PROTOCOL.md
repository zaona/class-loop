# Loop CanopusConnect 导入协议（LPC1）

Loop 0.3.0 起，AstroBox 插件通过 CanopusConnect 的 `loop` 模块通道直接与原生模块通信。无需导入快应用；旧版 interconnect JSON 握手不再使用。手机和设备共用 `crates/loop-protocol`，课表 schema 仍为 [version 1](SCHEDULE.md)。

## 数据流与限制

本地 ICS → 手机插件按 WakeUp / WeekDown / Nexio 方言转换 → LPC1 二进制分片 → 原生 Loop → 私有 `schedule.json`。插件需要 AstroBox NG API Level 4 的 Connect 支持；Loop 必须已激活。

当前仅支持提供 CanopusConnect 的 `.043` / `.155` / `.179` 三个固件目标。

- 每帧最大 480 字节：16 字节头 + 最多 464 字节 payload。
- 完整课表为 UTF-8 JSON，1～40960 字节；一次仅接收一个事务。
- 多字节整数均为 little endian；事务 ID 为非零 u32。

## 帧头

| 偏移 | 长度 | 字段 |
|---|---|---|
| 0 | 4 | ASCII `LPC1` |
| 4 | 1 | 操作码；响应置高位 `0x80` |
| 5 | 1 | 状态；请求固定 0 |
| 6 | 2 | 保留，固定 0 |
| 8 | 4 | transaction ID |
| 12 | 4 | value，含义见下表 |
| 16 | 可变 | payload |

响应回显操作码与事务 ID，payload 为空。

| 操作 | 码 | 请求 value / payload | 成功响应 value |
|---|---|---|---|
| HELLO | 1 | 0 / 空 | 40960（最大课表字节数） |
| BEGIN | 2 | JSON 总字节数 / CRC32 的 u32LE | 已接收偏移，首次为 0 |
| DATA | 3 | 当前字节偏移 / 非空 JSON 分片 | 下一分片偏移 |
| COMMIT | 4 | 0 / 空 | 已保存课程数 |
| ABORT | 5 | 0 / 空 | 0 |

CRC32 使用 IEEE 多项式 `0xedb88320`，初始值与最终异或均为 `0xffffffff`。标准向量 `123456789` 的结果为 `0xcbf43926`。

## 确认、重试与取消

手机按 HELLO → BEGIN → DATA… → COMMIT 停等发送，每收到匹配 ID、操作码和偏移的确认再推进。每阶段超时 5 秒，最多额外重试 3 次。过期或其他事务的确认会被忽略；失败或超时后尽力发送 ABORT。

相同 BEGIN 返回已接收偏移；相同偏移和内容的 DATA 可重复确认而不追加。设备缓存最近一次成功 COMMIT 的 ID 和课程数，确认丢失时重复 COMMIT 不会再次写入。不同活动事务返回 Busy；连续 60 秒无活动释放未完成数据。腕上清空课表会取消正在接收的事务。

状态码：0 Ok、1 Invalid（帧或参数错误）、2 TooLarge、3 Sequence（顺序或偏移错误）、4 Checksum、5 Schedule（JSON/schema/业务校验失败）、6 Io、7 Busy。错误响应的 value 通常为 0；部分 Sequence 响应携带设备已接收偏移。手机遇错误终止本次传输，用户可重新推送。

## 持久化与界面更新

设备完整收齐后检查长度、CRC32、schema 版本、有效日期、非空学期名，以及课程 ID、名称、星期、时间和教学周区间。先写模块私有 `schedule.json.tmp`，处理 short write，依次 sync、close，然后 rename 覆盖 `schedule.json`。只有全部成功才确认 COMMIT；失败清理临时文件，保留原课表。

实际正式路径为 `/data/canopus/m.loop/schedule.json`，文件服务调用使用相对路径。接收回调不直接操作 LVGL，成功后设置重载标记，由下一次 UI tick 读取新课表。旧快应用沙箱文件不自动迁移，请在手机插件重新导入。

共享协议测试覆盖重复请求、错误事务、越界、乱序、Busy、CRC 损坏、提交失败和取消；设备 mock 文件服务测试覆盖短写及 write/sync/close/rename 失败，并将真实课表通过共享协议送入实际保存函数。主机测试不替代蓝牙传输和断电情况下的真机验证。
