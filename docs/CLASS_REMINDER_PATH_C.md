# 上课提醒 Path C：RE 与落地计划

> 状态：**只做 C**（熄屏可达 + 震动/全屏 + 不黑屏卡死）。  
> 前置失败记录：[`CLASS_REMINDER_DEVLOG.md`](CLASS_REMINDER_DEVLOG.md)。  
> Target：`xiaomi-band-10-pro-3.101.036`（SHA `662d67f5…8ec3c81`）。

## 1. 产品定义（冻结）

| 项 | 选择 |
|---|---|
| 提前量 | 固定 10 分钟 |
| 通道 | 系统通知 + `start_reminder = 1`（震动/全屏） |
| 场景 | 熄屏后台也必须触达 |
| 硬约束 | 不得长时间黑屏卡死 |
| 不做 | Path A（仅前台）、Path B（静默无震动）作为最终交付 |

在找到合法 reminder 上下文之前：**不把 Loop reminder 业务代码合回 master**。先证据，再接线。

## 2. 已证明 vs 缺口

### 已有

- `lvx_notification_insert_message` + 88B 布局：`EVID-NOTIFY-001`（DEVICE_PROVEN）
- `start_reminder` @ +0x50 → reminder 路径 / event 32
- 久坐 builder 样板：`sub_C4B28AC`（完整填字段后 insert）
- Lyra 短 `bt_timer` 心跳可做**调度**（不是合法 reminder UI 上下文）
- Loop 经验：BT + `start_reminder=1` 能响但黑屏；UI timer 熄屏不跑

### 必须用 RE 回答（C 的阻塞点）

| ID | 问题 | 入口 |
|---|---|---|
| **Q1** | 久坐如何在熄屏触发 reminder？谁调用 `sub_C4B28AC`？定时源？insert 时线程/owner？ | `sub_C4B28AC` xrefs |
| **Q2** | event 32 / reminder UI 跑在哪个 FSM/线程？是否要求 display 已亮？ | `sub_CA81F10` 内 emit → handler |
| **Q3** | 能否从 BT（或传感器）安全 hop 到 `notification_owner` / 等价 UI owner？ | owner 字符串、queue/post API |
| **Q4** | 图标路径（PNG / NULL 默认 / Loop `.bin`）是否独立导致黑屏？ | `sub_CA81500` 默认资源；探针矩阵 |

对应 Canopus 候选证据（036 pack）：

- `EVID-REMINDER-001` — Q1 久坐调度与 insert 上下文
- `EVID-REMINDER-002` — Q2 event 32 / reminder UI owner
- `EVID-REMINDER-003` — Q3 cross-thread hop（有候选 API 后再填）

## 3. IDA 作业单（036 / `vela_ap.bin.i64`）

前提：本机需有与 SHA 对齐的 IDB。打开后按序：

1. 跳转 `sub_C4B28AC` → **Xrefs to**，标出所有调用者；对每个调用者继续向上，直到出现 timer / workqueue / alarm / sensor 边界。
2. 在 `sub_CA81F10`（`lvx_notification_insert_message`）内找 `start_reminder` / event **32** 发射点 → 跟到 handler。
3. 搜字符串：`notification_owner`、`reminder`、sedentary/久坐相关 log；记录创建/绑定 owner 的函数。
4. 对比 Manager 真机路径（`/dev/canopus` write → insert，`start_reminder=1`）与久坐路径的线程差异。
5. 每条结论写入对应 `EVID-REMINDER-00x`（`callsite_evidence` / `control_flow_evidence` / `ownership_analysis`），`verdict` 先 `CANDIDATE` → 审核后 `STATIC_RECOVERED`。

成功标准（静态）：

- 能画出「定时到期 → … → insert(start_reminder=1)」完整调用链；
- 标明 insert 与 reminder UI 各自的 thread/owner；
- 若存在 hop API，写出原型假设与 ownership（谁持有 queue、callback lifetime）。

## 4. 真机探针矩阵（静态有候选后立刻做）

变量：

- 上下文：`UI` | `BT` | `hop_to_owner`（若已恢复）
- `start_reminder`：`0` | `1`
- 图标：`PNG常驻` | `NULL默认` | `Loop.bin`

每格记录：熄屏是否触达、是否震动/全屏、是否黑屏卡死、`insert` 返回值。

禁止再盲试「仅 BT + start_reminder=1」作为交付方案。

## 5. 解锁后的 Loop 接线（暂不实施）

仅当 `EVID-REMINDER-001/002`（及必要时 003）达到可设备验证结论后：

1. 恢复 `loop-core` 提醒窗扫描（`next_reminder`，提前 10 分钟）。
2. 调度：短 `bt_timer` 心跳 **或** 库存等价调度（以证据为准）。
3. 投递：到期后经**证据批准的上下文**调用 `notification_insert`，`start_reminder=1`，图标用稳定 PNG（对齐 Manager）。
4. 去重：同课同日同 `start_min` 内存一次。
5. 真机 gate：熄屏触达 + 震动全屏 + 连续多次无黑屏卡死。

## 6. 当前阻塞（环境）

- 本机未找到 `vela_ap.bin.i64` / 036 固件 IDB。
- `canopus` CLI 未在 PATH（可选；证据可手写 JSON）。

下一步：提供 036 IDB 路径（或打开 IDA MCP）后，从 Q1 `sub_C4B28AC` xrefs 开始填 `EVID-REMINDER-001`。
