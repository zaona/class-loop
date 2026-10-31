# Loop UI 重写记录

本文为 0.3.0–0.3.2 的历史记录，所列自制 LVGL UI、像素布局和预览命令已移除。当前 0.3.4 使用系统预制组件，恢复 v1 文案、按钮和原生页面栈，见 [V1-PARITY-034.md](V1-PARITY-034.md)。

2026-10-31，按用户要求抛弃旧 UI 写法，对照 Lyra `ng@4dbb2eb` 重写。下文记录 0.3.0 / versionCode 3 的重写过程；当前已在 Canopus `ds-newdevice-supports@7995f192` 重建为 0.3.1 / versionCode 4，见 [新分支适配记录](DS-NEWDEVICE-SUPPORTS.md)。0.3.0 真机打开时出现 BusFault，新包尚待复测。

## 源码分层

| 文件 | 职责 | Lyra 参考 |
|---|---|---|
| `crates/loop-core/src/view.rs` | 五页 `PageView`、课程卡片和类型化操作 `Target` | `lyra-player-core/src/view.rs` |
| `crates/loop-core/src/layout.rs` | 336×480 和 212×520 面板的像素布局 | `lyra-player-core/src/layout.rs` |
| `crates/loop-device/src/target/ui/mod.rs` | 每页宿主、刷新定时器、事件和导航 | `lyra-player-device/src/target/ui/mod.rs` |
| `target/ui/lv.rs` | LVGL 对象/标签、MiSans 32/24、样式与事件封装 | `target/ui/lv.rs` |
| `target/ui/pages.rs` | 页面持有控件、差异刷新、分批创建与确认状态 | Lyra 歌单及播放器页面 |
| `target/ui/widgets.rs` | 圆角课程卡片、返回按钮、按下反馈 | Lyra 自定义控件组合方式 |
| `target/ui/motion.rs` | 短期动画时钟和淡入/平移 | `target/ui/motion.rs` |
| `tools/loop-ui-host` | 编译实际设备 UI，替换 LVGL 为主机记录器 | Loop 的测试工具 |

已删除 `loop-core/src/ui.rs` 和 `loop-device/src/target/ui_backend.rs`；core/device 移除 `canopus-ui-core` 依赖。UI 不再通过 Tree、Snapshot、节点编号或固件列表行渲染。卡片直接组合 LVGL 对象与标签，body 使用原生垂直滚动。

课程、日期、刷新和清空由类型化目标处理，课程 ID 保留完整 u32。页面事件先解析并复制操作，再进入导航，避免同步销毁源页面时仍持有可变引用。

Launcher 仍采用新版 Supervisor 服务分配的动态 app ID。Band 10 Pro 返回时传 live page descriptor，Band 11 传页面 key；没有复制 Lyra 当前固定 `0xCC` 的应用注册方式。课表、CanopusConnect 和存储逻辑继续使用当前 ABI 2 服务。

## 布局和更新

- 五页：首页、今日、本周、课程详情、数据。窄屏顶部和底部留出曲面空间。
- 卡片使用绿色强调、文字、圆角背景、按下反馈；首页显示当前课程进度。
- 列表课程名允许省略，详情页课程名、时间、地点和周次使用 LVGL 循环滚动，完整内容可读。
- 每 100ms 检查视图版本，仅在变化时生成展示数据；已有卡片按内容差异更新。
- 每 tick 最多创建或更新 4 张卡片，12ms 预算；滚动内容随实际建成的卡片增长，60 门课可全部到达。没有旧 UI 的 32 节点上限。
- 课表变化时立即解除旧输入绑定，事件再次核对 generation。暂停禁用输入，恢复保留已有滚动容器。
- 清空要求二次确认，5 秒过期；课表版本变化、暂停或销毁时取消确认，导入的新课表不会继承旧确认。
- 入场动画持续 240ms，33ms 动画定时器结束或暂停即释放；销毁同时释放刷新定时器和 Rust 状态。固件负责删除页面对象树。

## WSL 验证

在 `Ubuntu-24.04` 的 `/mnt/d/Canopus/class-loop` 中：

```sh
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host -- --check
cargo test --workspace
cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-139
sh scripts/build-wsl.sh
```

workspace 14 项测试通过（core 8、device 3、protocol 2、实际 UI 综合测试 1），布局导出为默认忽略的独立工具测试。实际 UI 测试覆盖 60 门课程分批创建、末项点击、动态应用 ID、两种返回参数、导入后旧点击失效、清空确认/超时/导入失效、无变化不写文本、暂停/销毁和创建失败时定时器释放。Band 11 feature 单独通过同一综合测试。

完整构建覆盖 `.036/.043/.139/.155`，包括官方 ELF verifier、`.canopus` 包、CMI2 安装器、包校验和 Lua 语法校验。日志在本地忽略目录 `build/migration-abi2/ui-rewrite-tests.log`、`ui-host-band11-tests.log` 和 `ui-rewrite-build.log`。

## 布局预览

预览直接导出上述设备 UI 创建的控件树，以 Pillow 和近似字体绘制。只用于检查几何、文案和裁剪，不能验证 LVGL 字体度量、动画、触摸或实际固件行为。循环滚动标签的预览只显示静态首帧。

```sh
sudo apt-get install python3-pil
LOOP_UI_PREVIEW="$PWD/build/migration-abi2/ui-preview-10pro.json" \
  cargo test -p loop-ui-host export_layout_preview -- --ignored
LOOP_UI_PREVIEW="$PWD/build/migration-abi2/ui-preview-band11.json" \
  cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-139 \
  export_layout_preview -- --ignored
python3 scripts/render-ui-preview.py \
  build/migration-abi2/ui-preview-10pro.json \
  build/migration-abi2/ui-preview-band11.json \
  --output build/migration-abi2/ui-preview.png
```

`LOOP_UI_PREVIEW` 使用绝对路径，因为 Cargo 测试的工作目录为工具 crate。绘图默认使用 Windows 的微软雅黑 `/mnt/c/Windows/Fonts/msyh.ttc`；可用 `--font` 指定其他本地字体。

构建时 `.036` 的 `.rodata+0xe774` 将填充字节和月份范围 `RangeInclusive<u8>` 的 `00 01 0c` 连读为 `0x0c010000`。已通过 ELF 符号、原始字节和 `parse_ymd` 的 relocation 核对，仅在应用清单添加这个精确数据常量；没有放行任何地址范围。反汇编保留于 `build/migration-abi2/ui-rewrite-disassembly.txt`。

尚无 Loop 真机或 Mirage 验证。需在设备上继续核对中文字体、长文本滚动、曲面边缘、页面返回、灭屏恢复、连续导入与销毁。Canopus `.155` Supervisor 上游文件预算超限仍独立存在，模块构建通过不表示 Supervisor 可部署。预算和固件符号批准范围保持原样。

## 后续固件与滚动处理对齐

用户要求补齐与 Lyra 相同的固件支持和滚动细节后，新增 Band 11 `.179`：清单、设备 feature、精确 SDK 绑定、UI 面板判断和原生页面生命周期均包含该目标。三个 Band 11 分支继续使用页面 key 返回，避免误用 Band 10 Pro 的 descriptor 路线。

滚动 body 在普通 `scrollbar_mode = OFF` 后调用与 Lyra 相同的 `lv::quiet_scroll_bar`。Band 10 Pro 的 `lvx_object_create` 实际调用 stock page-content 构造器；按 Lyra 对 `.036/.043` 构造器的分析，+60 的 `0x015b0102` 包含私有 scrollbar 模式、形状和角度，普通 LVGL scrollbar 设置不能控制这个私有模式。这里只将模式字节改为 0，保留相邻字段、滚动方向和可滚动 flag。Band 11 使用普通 LVGL 对象，`.139/.155/.179` 全部跳过结构写入。

主机记录器现为对象持有稳定的真实字节缓冲区，运行同一份设备结构写入，并核对只改变偏移 60、空指针不写、Band 11 不写、实际页面已调用该处理。WSL workspace 15 项测试通过（原 14 项加结构写入边界测试）；三个 Band 11 UI 分支分别通过综合测试与边界测试。复现命令：

```sh
cargo test --workspace
for firmware in 139 155 179; do
  cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-$firmware
done
sh scripts/build-wsl.sh
```

五目标 ELF、完整包、CMI2 安装器和 Lua 校验通过，日志为 `build/migration-abi2/firmware-alignment-tests.log`、`firmware-alignment-band11-{139,155,179}-tests.log`、`firmware-alignment-build.log`。当前完整包包含五个目标，1212416 字节。此前四目标重写日志保留为阶段记录；滚动行为与 `.179` 导入仍需设备回归。
