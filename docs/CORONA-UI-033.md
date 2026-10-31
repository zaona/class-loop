# Loop 0.3.3：管理器 / Corona 系统组件与教学周修复

本文是 0.3.3 历史记录。当前 0.3.4 已恢复 v1 文案、原生页面栈与 UI 线程定时刷新；本篇的单页替换、分页和刷新限制不再适用于当前版本，见 [V1-PARITY-034.md](V1-PARITY-034.md)。真实日期修复保留。

2026-11-01（UTC+8）。用户要求 UI 参考 Canopus 管理器与 Corona，不再自行设计视觉；同时反馈导入后总课程数量存在，但本周每天都是 0，教学周显示为 0。

## 参考源码与本地仓库

- Canopus：`../Canopus`，`ds-newdevice-supports@7995f19240c20b95228fa2b607fce0c3196eca37`。
- Corona：按用户要求完整克隆至 `D:/Canopus/Canopus-Module-Corona`，官方 origin 为 `https://github.com/AstralSightStudios/Canopus-Module-Corona.git`，commit `86ed7ed8700ccbb41825bcf8df21317f21a22916`。
- UI 依据：Canopus `manager/ui/canopus_manager_native.c`、`manager/target/canopus_ui_host.c`，Corona `src/manager_ui.c` 的 `row`、`render`、`on_open`、`on_back`、UI / Launcher 注册方式。
- Canopus、Lyra 和新克隆的 Corona 工作树均保持干净；未修改框架符号、批准状态、地址白名单或构建预算。

克隆在 WSL Ubuntu-24.04 中用 Linux Git 完成。WSL NAT 直连 GitHub 超时，且不能访问 Windows 的 localhost 代理；本轮临时使用只转发该公开仓库的本机 HTTP 桥，将请求交给 Windows `curl.exe` 经已有 `127.0.0.1:7897` 代理传输。Git clone、对象写入与检出均在 WSL；完成后关闭桥，并将 origin 恢复为官方 HTTPS 地址。没有永久改代理设置或安装额外服务。正常网络下的复现入口仍为：

```sh
cd /mnt/d/Canopus
git clone https://github.com/AstralSightStudios/Canopus-Module-Corona.git
```

## UI 改动

Loop 0.3.3 / versionCode 6 通过 `ctx.ui` 的 `navigation_page`、`navigation_header`、`text`、`section`、`action_row`、`status_row` 描述界面；原生标题栏、字体、列表行及滚动由与管理器相同的后端创建。没有新增自定义颜色、圆角、像素布局或手势控件。

阶段 1 注册 `UiEntryV1`，阶段 2 以空原生页面数组注册 Launcher；Supervisor 分配 app ID 并托管页面。模块标记改为 `NATIVE_APP_INTEGRATED`。五个业务页面保留，作为同一个托管页面内的路由；首页隐藏标题返回，系统右滑允许退出，子页标题返回和系统右滑退回上一业务页面。

不直接照搬 Corona 最新多页 `route` 路径：当前固定的 Canopus 基线在原生子页创建时没有向模块重新交付新的 UI context，不能假定新 Corona 依赖的多页行为已可用。这里使用 Corona 保留的单页替换契约；子页切换不提供独立的系统 push/pop 动画。

已移除：

- `target/ui/{lv,pages,widgets,motion}.rs` 中的自制卡片、文字返回键、进度条、动画与直接 LVGL 调用。
- `loop-core/src/layout.rs` 的固定面板像素几何。
- 直接写 Band 10 Pro 容器 `+60` 字节的应用侧滚动条处理。
- 旧 LVGL 主机记录器、布局导出、`scripts/render-ui-preview.py` 和不再存在的 UI 固件分支测试 feature。

课程列表使用系统 action row，状态信息使用 status row；首页上课进度显示为系统状态行。每页最多七项，超过后使用系统“上一页 / 下一页”行，避免超出框架 32 节点 / 1536 字节字符串限额。列表文案使用明确省略预览；课程详情采用系统分节与自动换行文字，长名称、地点分段分页保留全部字符。

## 第 0 周的原因与修复

当前框架 `canopus_supervisor_services.c` 中：

```c
static uint64_t svc_time_realtime_ms(const struct canopus_context_v1 *ctx)
{
    return (uint64_t)svc_time_monotonic_ms(ctx);
}
```

旧 Loop 把该开机时长解释成 Unix 日历时间，得到 1970 年附近的日期，早于学期开始日；`term_week` 因而返回 0。课程总数来自已保存数组，不受日期影响；本周课程则要求 `active_in_week(0)`，因此全部被过滤。这与用户反馈一致，主机回归能够复现此链路。

Loop 改用精确 target SDK 已有的 `canopus_fw_clock_gettime(CLOCK_REALTIME=0)`，接收 SDK `stock_timespec_t`，再转换为 Asia/Shanghai 日期、分钟和星期。没有修改上游时间服务或添加固件地址。主机测试验证 UTC+8 午夜边界：2026-10-31 16:00 UTC 为 11 月 1 日周日；以 2026-08-31 开学日计算为第 9 周，随后能够在周一页面看到导入课程。

真实日期读取仍需用户在设备上验收；手环自身时间与导入课表的学期起始日仍应正确。时间不可用、没有课表与学期未开始分别显示原因，不再统一显示“第 0 周”。

## 刷新与生命周期边界

当前 SDK 在托管页面暂停时可能释放 UI context，而不调用模块的 `on_close`。因此本版只在框架入口 `on_open`、`on_event`、`on_back` 提供有效 context 时绘制，不创建 UI 定时器，也不从异步任务访问保存的 UI 指针。

页面打开、交互或返回时读取日期与导入结果并刷新。导入后进入本周页面即可读取已有数据；页面静置时不自动重绘当前课程或进度。这是当前基线的明确限制，不能当作已实现每秒自动刷新。需要自动刷新时，应先补齐框架暂停 / 恢复的生命周期契约，再接入刷新任务。

框架校验语义事件 generation；Loop 在读取新课表后重新核对点击项对应的类型化目标。删除仍需二次确认，5 秒过期；导入或确认失效后不会沿用旧确认。构建失败或树提交失败保留旧树，待下一次交互重试。

## WSL 验证

```sh
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cd /mnt/d/Canopus/class-loop
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
sh scripts/build-wsl.sh
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
```

workspace 15 项测试通过，无忽略项。测试涵盖日期 / 周次、实际语义 UI 源码、SDK 树构造、60 项列表末项可达、课程详情完整 UTF-8 内容、导入后的旧目标失效、系统返回、清空确认与超时、失败提交后的树中止与重试，以及已有的存储原子性和 Connect 协议回归。

`.036/.043/.139/.155/.179` 五个 ELF 均通过官方 verifier、零未定义导入；完整包、`.043` 专用包、CMI2 安装器和 Lua 语法检查通过。设备编译无警告；host 测试仍有既有的 storage include 未使用函数警告。构建测试日志：`build/corona-ui-033/{tests,build-all,build-043}.log`。

这些是 host / static build 级结果，不能提升为真机 UI 或全部设备验证。此前“0.3.2 不再重启”的用户反馈也不能自动覆盖 0.3.3；仍需测试新包的显示、滑动、退出与重新打开。

## 最终产物与数据

| 产物 | 大小 | SHA-256 |
|---|---:|---|
| `dist/selected/3.101.043/loop-0.3.3-3.101.043.canopus` | 235008 字节 | `7dd3defea16a5bdf8cc0c34882e5b2c89e96ead14b61a0cc8412287efee56493` |
| `dist/loop-0.3.3.canopus` | 1165312 字节 | `4f41a5fcfb3fa26078bfe4b8a0ff052452b052dd2aabbe91071e20ab819e0862` |

`.043` ELF SHA-256：`8f4c8e97e939a0b0a84a7ace61b3244456cd152ac30ad70dd9ef2685ca46c4be`，resident 136708 字节。其余目标哈希及 SDK/CLI 信息在同名 `.build-info.json`。

课表仍位于 `/data/canopus/m.loop/schedule.json`；升级不清空已有数据，本次没有改存储格式、LPC1 协议或手机插件，已有导入无需因本次升级而重导。安装 versionCode 6 后按管理器提示重启生效，再检查真实教学周和各天课程内容。
