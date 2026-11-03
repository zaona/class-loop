# Loop 0.3.4：恢复 v1 文案、操作与原生切页

当前维护版本为 0.3.6 / versionCode 9，修复长课程信息撑高原生列表卡片的问题；此前审查补齐按下到松手期间的导入校验及切页失败恢复，并清理旧文件，见 [CLEANUP-REVIEW.md](CLEANUP-REVIEW.md)。本文保留 0.3.4 的功能基线、构建记录和包哈希；当时点击测试只覆盖 CLICKED 维护期间发生的导入，完整 PRESSED/timer/CLICKED 场景由 0.3.5 补上。

2026-11-01（Asia/Shanghai）。版本 0.3.4 / versionCode 7。功能基准为重构前提交 `6719914` 的 `crates/loop-core/src/ui.rs`、`loop-device/src/target/mod.rs` 和原生页面后端；组件与固件调用参照当前 Canopus 管理器、Corona 和 Lyra。

## 逐项对照

| 位置 / 操作 | 0.3.4 行为 |
|---|---|
| 首页标题 / 摘要 | Loop；学期名与第 N 周，无课表、时间不可用、未开学分别说明 |
| 现在 | 当前课名、节次、地点；无课程显示“暂无课程”，同 v1 为状态行，不跳详情 |
| 下一节 | 下一节课程信息；无下一节显示“没有下一节”，同 v1 为状态行 |
| 进度 | “今日剩余 N 节”，恢复 v1 含义，移除本次重写新增的百分比 |
| 今日课表 | 打开设备当天；课程按时间排列，显示节次 / 上课时间与地点 |
| 本周 | 显示“选择一天查看”，随后是周一至周日；当前版本已去掉额外显示的周数 |
| 七个星期按钮 | 显示“N 节”或“没课”；选择后打开对应日期，保留选择的星期 |
| 日课表空状态 | 恢复“今天没课”；有课显示“点选查看详情”；无课表 / 时间错误另行说明 |
| 课程按钮 | 打开所选课程，完整 u32 ID 不与按钮事件区间混用 |
| 详情 | 标题“详情”；课程名、时间、星期、地点、周次；无地点“—”，失效课程“未找到课程” |
| 数据管理 | 恢复“数据”页以及“管理已导入的课表”/ 实际操作状态 |
| 刷新课表 | “立即重读文件”；重新读取已保存课表，显示刷新结果 |
| 清空本地课表 | “删除已导入数据”；点击即清空，恢复 v1 行为，移除新增的二次确认和取消 |
| 标题返回 | 结束当前原生页，回到实际来源页面；详情从哪一天打开就返回哪一天 |
| 系统右滑 | 交给固件默认页面返回；恢复时同步业务历史，首页允许退出 |
| 长列表 | 连续滚动；移除七项一页的“上一页 / 下一页”按钮，不丢弃列表末项 |
| 导入发生在点击期间 | 先核对旧按钮对应课程是否仍一致；变更则刷新列表，不误开新的课程 |
| 灭屏、亮屏、退出、重开 | 暂停停止定时器，恢复重建一个定时器，销毁清除所有本页原生句柄 |

“课间时光”“课表，随时在腕上”“一周安排”“享受你的时间”、新增的手机导入状态项等文案均已移除。新架构的 CanopusConnect、模块私有存储、动态 app ID 和真实日历时钟修复保留。

## 页面与组件

0.3.3 使用一个 Supervisor 托管页面替换业务树，因此内部跳转不会触发固件页面栈动画。本版恢复五个原生描述符，通过现有 `ctx.launcher.register_app` 注册；Supervisor 分配动态 8-bit app ID，模块不调用固定 ID 的 app_install，不修改上游 SDK。

每次前进调用固件 activity_navigate；标题返回调用 activity_finish。Band 10 Pro 传固件交付的 live page descriptor（可能不同于注册时静态描述符），Band 11 传 app/page key。周页编号 2 到日页编号 1 仍然是 push，方向与页面编号大小无关。系统手势不抢占固件默认 finish，所以使用其原生返回效果。

生命周期对照 Lyra 当前源码：Band 10 Pro 生成结构中名称已陈旧，+0x5c 是 PAUSE、+0x64 是 UI_DESTROY；Band 11 使用其 on_pause / on_ui_destroy 字段。页面暂停保留固件原生对象以供切页动画，停止 UI 线程定时器，恢复后重新刷新。没有恢复原来的自制卡片、像素绘图和应用侧过渡动画。

标题、行、正文使用管理器同类 LVX 系统预制组件和默认样式；不依赖 `ctx.ui` 托管子页上下文。摘要是独立的原生文字对象，并按实际文本为两行周次 / 操作提示预留高度。Band 11 的文字、箭头触点参照管理器 `band11/canopus_band11_abi.h`；只读取已恢复的子对象句柄，不写私有对象字段。

不需要新管理器包。本版继续使用 `ds-newdevice-supports@7995f19240c20b95228fa2b607fce0c3196eca37`，Canopus / Lyra / Corona 的上游源码保持干净；没有提升符号批准状态、增加固件地址或扩大 verifier 范围。

## WSL 复现与证据边界

所有命令在 Ubuntu-24.04 / root 运行，源码 `/mnt/d/Canopus`。

```sh
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cd /mnt/d/Canopus/class-loop
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-155
sh scripts/build-wsl.sh
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
```

日志存于 `build/functional-parity-034/{tests,tests-band11,build-all,build-043}.log`。主机测试运行实际设备端 native_app 和 ui 源码，模拟固件页面调度、控件、heap 句柄与定时器；覆盖 60 门课程、所有按钮与七个星期、首次周次提示占位、标题 / 系统返回、长 UTF-8 内容、灭屏暂停恢复、退出重开、导入时点击失效。核心另测 u32::MAX 课程 ID，并保留存储原子性、真实日历日期、Connect 协议回归。

最终结果：workspace 14 项通过，Band 11 原生 UI 分支另 1 项通过，均无忽略项。五个固件 ELF、完整包、`.043` 专用包、CMI2 安装器和 Lua 语法检查通过；全部 ELF 为零未定义导入。设备构建无警告，host 仍保留 storage include 的既有未使用函数警告。`.043` resident 为 135872 字节。

这些结果属于 host / static build 证据。实际动画是否流畅、固件对长文字的换行以及真机灭屏恢复仍需安装后验收，不能据此宣称已在设备验证。0.3.2 的“不再重启”反馈不自动覆盖本版本。

## 安装与数据

当前设备用 `dist/selected/3.101.043/loop-0.3.4-3.101.043.canopus`；完整五目标包为 `dist/loop-0.3.4.canopus`，同名 build-info 记录哈希与 SDK 来源。升级按管理器提示重启生效。

| 产物 | 字节数 | SHA-256 |
|---|---:|---|
| `.043` 专用包 | 231936 | `9d87402ad9401f344bb1c5c511200bda3fc83ad3cbe04dce81f773593af7e50a` |
| 五目标完整包 | 1151488 | `5d0ce5c3cbc415a8cc5f7c17b8d2fb49cb30da1c52c9bedd74013c23dc30aedd` |

课表仍在 `/data/canopus/m.loop/schedule.json`，格式、手机插件与 LPC1 协议不变。已有数据保留，无需重新导入，也不需要替换现有管理器。
