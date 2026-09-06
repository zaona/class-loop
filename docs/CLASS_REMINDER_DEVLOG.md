# 上课提醒功能：开发过程记录

> 状态：**已回退代码**（2026-09-06）。本文仅作备忘，便于以后重新评估。  
> 相关对话：Cursor agent transcript `574171fc-826a-4d4f-86e4-c1cbc43b49e4`。

## 1. 目标

在 Loop（Canopus 原生模块，`resident-after-activation`）上实现：

- 课表提前 **10 分钟**提醒  
- 走系统通知 `lvx_notification_insert_message`  
- **熄屏后台**也能提醒（完整版 B，非仅前台）  
- 期望：**震动 + 全屏 reminder**（`start_reminder = 1`）

产品默认（当时拍板）：

| 项 | 选择 |
|---|---|
| 场景 | 后台完整版 |
| 提前量 | 固定 10 分钟，暂无腕上开关 |
| 通道 | 系统通知 |
| 去重 | 同课同日同 `start_min` 只提醒一次（内存） |

## 2. 仓库与 Canopus 基线（调研结论）

### 已有、可用

- `loop-core`：`now_and_next` / `term_week` / 课表 JSON 加载  
- Canopus：`notification_insert` → `lvx_notification_insert_message`（Manager 真机验证，EVID-NOTIFY-001）  
- `firmware_notification_message.start_reminder`：非 0 走 reminder 路径（震动/全屏相关）  
- Lyra：`bt_timer_add` + `bt_queue_external` 短周期心跳做后台播放  

### 缺口 / 约束

- Loop 页内 `lvx_timer` 在 `on_pause` 停刷，**不能**扛后台提醒  
- `notification_insert` promotion：**UI owner thread**；allowed contexts：`notification_owner` / `system_app_lifecycle_callback`  
- `bt_timer_add`：**插入时不唤醒休眠 FSM**；长 delay 不可靠  
- 无公开震动 / 亮屏 / wake lock API  
- 一期 README 原写「不做上课提醒」

参考文件：

- `Canopus/.../firmware_notification_message`  
- `Canopus/targets/.../evidence/EVID-NOTIFY-001.json`  
- `Canopus-App-LyraPlayer/.../audio_service.rs`  
- Manager：`canopus_manager_target_lvgl_v9.c`（`start_reminder = 1`）

## 3. 实现骨架（曾合入工作区、后已撤销）

### 3.1 `loop-core`

新增 `reminder.rs`：

- `DEFAULT_LEAD_MINUTES = 10`  
- `next_reminder(courses, term, clock, lead, after)`  
- 提前窗：`[start - lead, start)`；`lead=0` 为开课当分钟  
- `after`：时间序严格晚于已提醒键，避免同窗重复  
- 扫描今日起最多 8 天；单测覆盖窗内/窗外/跨天/周次  

### 3.2 `loop-device`

新增 `target/reminder.rs`，对齐 Lyra：

- `activate` 后 `bt_queue_external` → 固定 **5s** `bt_timer_add` 链  
- 每拍读 `schedule.json` + 时钟，到期则 `notification_insert`  
- 数据页「测试通知」：`debug_notify()`（UI 线程）  

接线：`activate` / 课表刷新 / `deactivate`（非驻留路径）`stop`。

## 4. 真机迭代（核心矛盾）

需求上同时要：

1. **熄屏也能收到**  
2. **震动 + 全屏 reminder**（`start_reminder=1`）  
3. **不长时间黑屏卡死**

实测大致三角关系：

| 方案 | 熄屏能收到 | 震动/全屏 | 黑屏卡死 |
|------|------------|-----------|----------|
| BT 线程 + `start_reminder=1` + Loop `.bin` 图标 | 是 | 是 | **是** |
| 排队 → UI `lvx_timer` 投递 + `start_reminder=1` | **否**（亮屏才弹） | 是（亮屏时） | 否 |
| BT 线程 + `start_reminder=0` | 是 | **否** | 否 |
| BT + `start_reminder=1` + 图标留空 + 发出后静默 ~60s | 待充分验证 | 意图恢复震动 | 意图减轻卡死 |

### 4.1 阶段 A：长 delay 分片（1min / 空闲 5min）

- 问题：`reschedule` bump generation **打断定时器链**；长 delay 与 FSM 休眠不兼容。  
- 对照 Lyra 后改为 **5s 短心跳连续重装**。

### 4.2 阶段 B：BT 直接 `start_reminder=1`

- 结果：功能「能响」，熄屏有震动，但 **界面卡住黑屏，过一会才能点亮**。  
- 判断：reminder UI / event 32 路径不宜在 BT owner 回调里跑（且可能叠加错误图标资源）。

### 4.3 阶段 C：BT 只排队，UI 定时器投递

- 首次打开 Loop 时建常驻 `lvx_timer`。  
- 结果：熄屏 **没有通知**；**点亮屏幕一瞬间才弹**（UI 定时器熄屏不跑）。

### 4.4 阶段 D：BT + `start_reminder=0`

- 结果：熄屏可收到、不黑屏，但 **无震动、无全屏**（用户明确不满意）。

### 4.5 阶段 E：BT + `start_reminder=1`，图标改固件默认，发出后静默

- 假设：黑屏与把 `/data/canopus/appicon_loop.bin` 塞进 reminder 大图路径有关（EVID：路径稍后由通知 UI 打开；Manager 用 PNG）。  
- 图标指针置空，走固件默认资源；成功后 `QUIET_TICKS` ~60s。  
- **未调到用户满意即决定回退整功能。**

## 5. 调试手段

- 数据页「测试通知」：UI 线程插入，状态显示 `result=`（成功常见 0/1）。  
- 测试 ICS（WakeUp 方言），例如周日晚某分钟开课，提前 10 分钟进窗。  
- `docs/BUILD.md` 曾写真机核对清单（已随回退删除）。

## 6. 未解问题（以后重做必看）

1. **如何在熄屏时从正确上下文触发 `start_reminder=1`？**  
   - UI timer 熄屏不跑；BT 线程跑 reminder UI 易黑屏。  
2. 是否存在 **亮屏 / 震动** 独立 API，可先 `start_reminder=0` 入队再补交互？当前 target-private **未暴露**。  
3. `notification_owner` 线程如何进入？能否从 BT 安全 hop 过去？  
4. Loop 启动器 `.bin` 是否绝不能用于 notification 大图？需 PNG 常驻路径实验。  
5. 发出 reminder 后与 **5s BT 心跳**是否互相抢跑？静默间隔是否足够。  
6. 模块仅 activate、从未打开页面时，有无 UI 投递通道。

## 7. 建议的后续路线（未实施）

1. 先做 **探针模块**：仅 UI / 仅 BT / 有无图标 / `start_reminder` 0|1 矩阵，记录黑屏与震动。  
2. 向 Canopus 要：`notification_owner` 投递或「亮屏后再 reminder」的正式语义。  
3. 若只能二选一：产品需明确优先「熄屏必达」还是「震动全屏」。  
4. 再考虑腕上开关 / 提前量配置。

## 8. 回退说明

回退内容（相对 `master` / `f7044cc` 工作区未提交改动）：

- 删除 `crates/loop-core/src/reminder.rs`  
- 删除 `crates/loop-device/src/target/reminder.rs`  
- 恢复 `loop-core` / `loop-device` / `README.md` / `docs/BUILD.md` 中提醒相关接线  
- 删除本地测试 ICS（如 `test-sunday-*.ics`）  
- **保留**本文档  

未改动已推送的 ICS 多来源插件提交（`f7044cc`）。

## 9. 关键符号速查

```text
notification_insert(message)          // UI owner / notification_owner
message.start_reminder != 0           // reminder path + 可能 event 32
bt_timer_add / bt_queue_external      // Lyra 后台心跳模式
lvx_timer_*                           // 页/UI 线程；熄屏不可靠
```
