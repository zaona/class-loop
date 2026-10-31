# Loop 课表数据格式（schedule.json）

设备端读取 JSON，不解析 ICS。课表由 AstroBox 插件从本地 `.ics` 转换后，通过 CanopusConnect 直接推送至原生模块。
**固件不含内置课表。**

## 谁写入、谁读取

```text
课表.ics（本地文件来源）
        │
        ▼
AstroBox 插件 plugins/astrobox-loop-import（按 ICS 来源方言转换，支持 WakeUp / WeekDown / Nexio）
        │ CanopusConnect（module = loop，二进制分片）
        ▼
Loop 原生模块校验并原子写入
        ▼
/data/canopus/m.loop/schedule.json
        │ 下一次界面刷新时加载
        ▼
loop-device / loop-core
```

- 无额外手表快应用；旧快应用工程已移除。旧沙箱文件不自动迁移，请重新推送。
- 手机插件：`plugins/astrobox-loop-import`（协议见 [SCHEDULE_IMPORT_PROTOCOL.md](SCHEDULE_IMPORT_PROTOCOL.md)）
- 文件缺失或非法：Loop 显示空课表（「尚未导入课表」），不回退样例

## Schema（version = 1）

```json
{
  "version": 1,
  "term": {
    "name": "2026秋",
    "start_date": "2026-08-31"
  },
  "courses": [
    {
      "id": 1,
      "name": "C语言程序设计",
      "location": "啬园校区 JX01-402",
      "weekday": 2,
      "start_min": 470,
      "end_min": 560,
      "period_label": "第1-2节",
      "weeks_start": 1,
      "weeks_end": 17,
      "week_interval": 1
    }
  ]
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `version` | 是 | 固定 `1` |
| `term.name` | 是 | 学期显示名 |
| `term.start_date` | 是 | `YYYY-MM-DD`；该日所在周 = 第 1 教学周 |
| `courses[].id` | 是 | 稳定正整数 |
| `name` | 是 | 课名 |
| `location` | 否 | 地点 |
| `weekday` | 是 | 1=周一 … 7=周日 |
| `start_min` / `end_min` | 是 | 当天 0 点起的分钟（权威时间） |
| `period_label` | 否 | 如 `第1-2节`，仅展示 |
| `weeks_start` / `weeks_end` | 是 | 生效教学周闭区间 |
| `week_interval` | 否 | 默认 `1`；`2` 表示隔周（相对 `weeks_start`） |

生效判定：

```text
week ∈ [weeks_start, weeks_end] 且 (week - weeks_start) % week_interval == 0
```

## ICS → JSON

由 `plugins/astrobox-loop-import` 按 ICS 来源方言**独立**转换（WakeUp / WeekDown / Nexio）。
三源对同一人课表应得到一致的课名、星期、起止分钟、地点（去校区前缀后）与教学周区间。
重新推送课表无需重编 Loop。完整 JSON 上限 40960 字节；设备验证日期、课程时间、星期及教学周，失败保留原课表。当前 Connect 服务目标为 `.043` / `.155` / `.179`，`.036` / `.139` 不支持导入。
