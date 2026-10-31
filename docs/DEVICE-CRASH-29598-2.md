# Loop 重复 BusFault：第二份日志与诊断版

分析/构建日期：2026-11-01（UTC+8）。框架仍为 Canopus `ds-newdevice-supports@7995f192`。用户确认同一设备的 Canopus 管理器和 Lyra 均能正常打开；Loop 仍重启。

## 新证据

用户文件：`C:/Users/zaona/AppData/Local/moe.astralsight.astrobox/logs/device/29598_log.tar (2).gz`。在 WSL 中按上一轮的普通文件/路径检查规则解压到 `build/device-logs/29598-2/`。

`data/dfx/note/20261031232254_15725_0_0.ev` 继续追加了异常记录，其中以下三条为 `miwear` 的 BusFault，断言仍为 `armv8-m/arm_busfault.c:113`：

| 时间（UTC+8，2026-10-31） | 记录偏移 |
|---|---|
| 23:29:25.337952 | 0x86a |
| 23:30:27.352844 | 0xaad |
| 23:52:39.691773 | 0x150d |

三条非零回溯项与上一份日志的 23:26:57 相同：

```text
0x0c2034f4 0x0c1a0bc6 0x0c2034da 0x0c19b97a 0x1c028fb0
```

最新一次与 `tmp.log` 中 15:52:39 的 bl2 崩溃启动、15:52:43 的 AP 崩溃启动记录对应。这是同次重启的两个启动阶段。日志仍无完整异常寄存器、CFSR/BFAR 或 Loop 版本标识；不能单凭归档确认安装中的 ELF 哈希。

Lyra 正常使排查重点进一步集中在 Loop 页面初始化、父对象和控件组合；不能由此证明每条共用固件调用或每个资源状态都无问题。`0x1c028fb0` 仍只作为对象 class allocator 路径附近的回溯线索，不能提升为已确认的 fault PC，也没有证据归因于滚动条写入。

## 0.3.2 / versionCode 5 诊断版

本版加入启动阶段记录，不声称修复 BusFault，不修改固件地址、符号批准状态或 Canopus/Lyra 源码。

- 通过 `ctx.fs` 写 `/data/offlinelog/loop-startup.log`；上游 FS 允许该绝对路径，无需新增裸固件调用。该目录已出现在本设备日志归档中。
- 每行标识 Loop 版本、精确 target、序号、单调时间、阶段和两个指针/数值 token。不会解引用这些 token；页面自身 root 字段按既有精确描述符读取。
- 在激活、注册阶段、页面创建、容器/标签构造、样式/字体/尺寸/位置、滚动条模式修改、首次维护/课表刷新和定时器创建处记录 before/after。
- 固定 192 字节缓冲区，不分配堆内存。每条记录处理短写，尝试 sync，然后 close，完成后才执行下一步 UI 调用。IO 错误不改变应用控制流。
- 追加日志保留重启前证据；每次模块激活最多 512 条，文件最多 128 KiB，达到任一限制即停止记录。不会持续写入日常 UI 刷新，也不修改课表数据。

诊断 IO 会影响启动耗时和动画节奏，不作为性能基线。若只有 before 而无 after，它只是下一步核对的区间线索；日志 IO 失败也会丢记录，仍需结合异常栈判断。

## WSL 验证与产物

```sh
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/.cache/canopus/host"
cd /mnt/d/Canopus/class-loop
cargo fmt -p loop-core -p loop-device -p loop-protocol -p loop-ui-host
cargo test --workspace
for firmware in 139 155 179; do
    cargo test -p loop-ui-host --features target-xiaomi-band-11-4-100-$firmware
done
sh scripts/build-wsl.sh
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
```

workspace 16 项通过、1 项按需布局导出跳过。新增测试执行实际诊断写入函数，检查旧日志保留、短写、同步/关闭、写入/同步/关闭失败、文件容量和记录额度。三个 Band 11 UI 分支各 2 项通过、1 项布局导出跳过。五个 ELF、完整包、`.043` 单目标包和安装器 Lua 均通过校验，零未定义导入。

优先使用 `.043` 专用包：

`dist/selected/3.101.043/loop-0.3.2-3.101.043.canopus`

237568 字节，SHA-256 `79a7c518e74dd81109c5cd2ee53049feee5ac645b23f4c62d3dfaca4dd042657`。其中 `.043` ELF 为 139788 resident 字节，SHA-256 `057522343d3cdd6237b2c18a0f864cc02266eafa625a2e123ebce7f8e44b0e4b`。

完整五目标包 `dist/loop-0.3.2.canopus` 为 1177600 字节，SHA-256 `cb1916a3a43d9466b78ac192ff0145271730de4a9dd9e978f0368218279538f6`。同名 `.build-info.json` 记录当前 SDK/CLI 和所有 ELF 哈希。

构建测试日志在 `build/diagnostics-29598-2/`。旧 0.3.1 包、从旧包取出的 `.043` ELF、诊断包与 ELF 保留在 `build/device-logs/29598-2/binaries/`，避免继续构建覆盖证据。

## 下一次设备复现

1. 安装 0.3.2 / versionCode 5；若管理器要求重启生效，先完成重启并按原方式加载 Supervisor。
2. 打开一次 Loop。无论是否重启，随后导出设备日志，检查其中 `/data/offlinelog/loop-startup.log` 的 `v=0.3.2` 记录。
3. 对照最后连续的阶段、对象指针和新异常栈定位。若无该文件或版本不符，先确认升级是否真正生效及诊断文件是否被采集，不把旧回溯当成新版本的执行证据。

不需要为这次 Loop 诊断替换已经能正常打开的 Lyra，课表仍保存在 `/data/canopus/m.loop/schedule.json`。
