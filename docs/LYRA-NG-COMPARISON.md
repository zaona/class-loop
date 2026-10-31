# Loop 与 Lyra ng 的结构对照

2026-11-01 更新：当前 0.3.5 使用管理器同类系统预制组件，按 v1 恢复文案与操作，并参考 Lyra 原生生命周期执行固件切页；另增加旧点击校验和切页失败恢复。功能基线见 [V1-PARITY-034.md](V1-PARITY-034.md)，最新清理及修复见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)。后文 0.3.0 的绘图细节保留为历史。

核对日期：2026-10-31。参考相邻 `Canopus-App-LyraPlayer` 仓库当前检出的 `ng` 分支，commit `4dbb2eb`；框架已改为 Canopus `ds-newdevice-supports@7995f192`，见 [新分支适配记录](DS-NEWDEVICE-SUPPORTS.md)。

Loop 已接入当前 Canopus 架构：`core/view` 提供展示数据，`device/target/ui` 创建管理器同类系统组件。课表业务、导入协议和主机验证工具各有自己的实现。

## 已对齐的基础

| 项目 | Loop | Lyra ng |
|---|---|---|
| 构建清单 | 根目录 `Canopus.toml`，schema 1 | 相同 |
| 模块类型 | Rust、`kind = "target"`、ABI 2 | 相同 |
| 生命周期 | `resident-after-activation` | 相同 |
| Cargo 分层 | `loop-core` + `loop-device` + `loop-protocol` | `lyra-player-core` + `lyra-player-device`，另有导入组件 |
| SDK 依赖 | 相邻 Canopus 的 `sdk/rust` 路径依赖 | 相同 |
| 设备产物 | `staticlib`、精确固件 feature | 相同 |
| ARM 配置 | Cortex-M33、`thumbv8m.main-none-eabi` | 相同 |
| release 配置 | `opt-level=z`、fat LTO、单 codegen unit、abort panic | 相同 |
| 有效新版构建路线 | 官方 `canopus build` + `canopus package` | 相同，Lyra README 也指定这一入口 |
| 安装回执 | 官方 CLI 生成 CMI2 | 官方 CLI 生成 CMI2 |

## 目录和流程差异

| 项目 | Loop | Lyra ng |
|---|---|---|
| workspace 成员 | `loop-core`、`loop-device`、`loop-protocol`、`tools/loop-ui-host`，共 4 个 | player core/device、protocol、transfer、`tools/lyra-mirage-import`，共 5 个 |
| 手机到手表导入 | `plugins/astrobox-loop-import` + CanopusConnect；共享 `loop-protocol` 实现分片与确认 | `plugins/astrobox-lyra-import` + CanopusConnect，另有协议与传输 crate |
| 测试辅助目录 | `tools/loop-ui-host` 直接引入设备 UI 源，测试事件和生命周期；本地 `fixtures/`（忽略） | `tools/` 和多组 `scripts/mirage_*.py` |
| 目标清单 | `.036`、`.043`、`.139`、`.155`、`.179` | 相同五项 |
| 安装器入口 | `scripts/build-wsl.sh` → `scripts/build.py` → 官方 CLI | README 直接调用官方 CLI；未新增本地 WSL wrapper |
| 旧构建脚本 | 已移除旧入口与重复 ELF 校验脚本，统一 `build-wsl.sh` | `build-install-watchface*.sh` 仍有 CMI1、签名和本地 ctor 路线，不能照搬到当前流程 |
| 应用图标源 | `crates/loop-device/assets/appicon_loop.png` / `.bin`，由设备 crate 内嵌 | `crates/lyra-player-device/assets/appicon.bin`，由设备 crate 内嵌 |
| UI 代码 | `core/view.rs` + `target/ui/mod.rs`，五个系统组件原生页 | `core/{view,layout}.rs` 与 `target/ui` 按播放器、歌单、歌词等业务组织 |
| Rust 版本声明 | workspace 最低 1.88，版本 0.3.5 | workspace 最低 1.85，版本 0.1.1；模块清单版本为 0.2.1 |
| 校验常量 | 图标及当前编译产物的数据常量精确列表 | 音乐图标及其编译产物的独立列表 |

Loop 的有效源码目录为 `crates/`、`tools/`、`docs/`、`plugins/` 和 `scripts/`。旧 `targets/*.env` 和重复表盘图标已删除；`watchfaces/` 仅保留历史说明与忽略的旧产物。目标清单统一在 `Canopus.toml`，图标归入设备 crate。`build/`、`dist/`、`target/` 是本地产物和缓存目录，其存在不代表受版本控制的架构一致。

## 运行时差异

当前 Lyra `crates/lyra-player-device/src/target/native_app.rs` 使用固定 `APP_ID = 0x00CC`，由模块调用固件的 `app_install` 和 `launcher_add`。

Loop 通过 `ctx.launcher.register_app` 注册五页并接收 Supervisor 分配的动态 ID；应用身份与后续发布由 Supervisor 管理。文件、内存和时间通过 context 服务表访问。因此，两者在 ABI 和模块类型上对齐，原生应用注册实现仍有区别。

Loop 参考了 Lyra 的 Band 10 Pro 暂停/UI 销毁槽位修正，以及返回时保存固件传入 live page 指针的做法。Loop 的真机行为仍待验证，不能继承 Lyra 文档中的模拟器验证结论。

## UI 与组件对照

| Lyra ng 实现 | Loop 对应实现 |
|---|---|
| `core/view.rs`、`core/layout.rs` | 课表 `PageView/Card/Target` 和两种面板的像素几何，不含 UI 树或固件指针 |
| `target/ui/lv.rs` | 对照其对象/标签、MiSans 字体、样式属性顺序和固件事件常量；详情长文本使用循环滚动 |
| `target/ui/mod.rs` | 每页 create/resume/pause/destroy、100ms 刷新、先复制事件目标再导航 |
| `target/ui/motion.rs` 与页面动画 | 33ms 短期定时器，240ms 淡入/平移，暂停和销毁时释放 |
| 歌单的分批创建与差异刷新 | 课表每 tick 最多创建/更新 4 张卡片，12ms 预算，保留原生滚动容器 |
| 播放器、封面、歌词与音乐按钮 | Loop 自己的课程卡片、返回按钮和上课进度条，直接组合相同的 LVGL 基础控件 |

旧 `loop-core/src/ui.rs` 和 `target/ui_backend.rs` 已移除，core/device 均不再依赖 `canopus-ui-core`。完整实现及验证边界见 [UI-REWRITE.md](UI-REWRITE.md)。

## 重写后仍未对齐的具体项

“完全重写”指旧 UI 后端已替换；当前仍没有做到 Lyra 的逐文件或逐行为复刻。以下由重写后的源码重新核对：

| 项目 | 当前差距与后续工作 |
|---|---|
| Mirage 回归与性能证据 | Lyra 有启动/安装/导入/页面/截图/性能脚本及模拟器结果。Loop 的主机 LVGL 记录器不执行真实固件绘制或手势，也没有传输吞吐和设备 CPU/堆实测。两者都仍需真机验证。 |
| 长文本策略 | Lyra 当前歌名、艺人使用 `LONG_DOT`，其性能记录说明跑马灯持续重绘的代价。Loop 详情课程名、地点、周次使用循环滚动；这项尚未对齐其性能策略，需在完整内容可读与绘制成本之间重新设计并实测。 |
| 动画职责分层 | Lyra 把纯定点缓动放在 `core/motion.rs` 并单测，device `ui/motion.rs` 管理定时器。Loop 的单一入场缓动与时钟都在 device 层，缺少同样的纯函数分层与边界测试。 |
| 控件资源与自动检查 | Lyra 有 SVG → glyph 资源生成流程，并测试图标中疑似地址像素是否全部列入清单。Loop 使用文字返回符号和内嵌应用图标，目前靠资源编码及官方 verifier 核对，没有同样的图标清单自动检查。 |

需要保留的差异包括：Loop 的 Supervisor 动态 Launcher 注册和服务表内存分配（Lyra 当前固定 ID、直接 `bt_alloc`）；课表卡片与音乐控件的业务差异；课表协议的单文件停等传输（Lyra 是多媒体资源协议，另有 transfer crate）。这些差异本身不表示新版 Canopus 适配缺失。

按用户要求，`.179` 目标和固件滚动处理现已对齐：Loop 同步增加清单、Cargo feature、精确 SDK 绑定、Band 11 屏幕与生命周期分支；滚动区使用与 Lyra 相同的 `quiet_scroll_bar`，仅对 Band 10 Pro stock page-content 的 +60 模式字节写 0，三个 Band 11 固件均跳过。依据为 Lyra `ui/lv.rs` 的构造器分析，以及 SDK 中 Band 10 Pro `lvx_object_create → lvx_content_create`、Band 11 `lvx_object_create → lv_obj_create` 的不同实现。

WSL workspace 15 项测试、Band 11 `.139/.155/.179` 三个 UI 分支、五目标 ELF/包/安装器校验通过；主机测试实际检查写入范围和页面滚动容器的调用。Loop 的真机滚动与导入仍未验证，未继承 Lyra 的模拟器结论。

实际固件 UI、导入回归与长文本表现仍需设备验收。旧文件清理已按用户授权完成，范围见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)；历史产物和调查日志保留。

## 如何维护

- 后续以 Canopus SDK、迁移指南和官方 CLI 为接口依据；Lyra 作为参考应用，需要按当前文件内容判断其新旧流程。
- 新增导入协议或模拟器工具时，再按功能需要增加 crate 或 `tools/`，无需为目录外观复制音乐业务组件。
- 更新 Lyra commit 后重新核对本表；这里记录的是 `4dbb2eb` 的快照。

开发过程见 [WSL 开发与构建流程](WSL-DEVELOPMENT.md)，本次适配结果见 [ABI 2 迁移记录](MIGRATION-ABI2.md)。
