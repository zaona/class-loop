# 打开 Loop 后重启：29598 日志分析

分析日期：2026-10-31。用户确认触发操作为打开 Loop；Canopus 管理器可正常打开，Lyra 尚未测试。本次只分析日志，未修改运行时代码，也未生成修复包。

## 结论及边界

设备发生了 BusFault，随后崩溃重启。最近一次异常发生在 `miwear` 线程，回溯包含固件 LVGL 对象分配/创建路径附近的地址 `0x1c028fb0`。结合用户操作，优先排查 Loop 页面创建时传入的对象、底层 UI 调用 ABI 以及内存状态。

目前没有完整异常寄存器、CFSR/BFAR、模块装载地址或 Loop 启动阶段日志，因此尚不能确认是哪一个控件、哪一条调用出错。`0x1c028fb0` 是回溯项，不能直接当作已确认的 fault PC；也不能据此判断为课表数据损坏、堆耗尽或滚动条偏移写入导致。主机 UI 测试不能验证真实固件的对象布局、内存状态和调用 ABI。

## 日志证据

原始文件：`C:/Users/zaona/AppData/Local/moe.astralsight.astrobox/logs/device/29598_log.tar.gz`。

WSL 解压目录：`build/device-logs/29598/`（忽略的本地产物，不提交设备日志）。

日志中的资源模块启动记录指向 `xiaomi-band-10-pro-3.101.043`。该记录的 `version=0.3.0` 属于资源模块，不能用来确认设备上安装的 Loop 版本。

`data/dfx/note/20261031232254_15725_0_0.ev` 包含两条 BusFault 断言。以下时间由记录偏移 +8 的 little-endian u64 纳秒时间戳转换为 UTC+8；不能用文件名时间代表每条记录时间。

| 时间（UTC+8） | 任务 | 记录起点 | 断言 |
|---|---|---|---|
| 23:23:09.267090 | system | 0x1bd | armv8-m/arm_busfault.c:113 |
| 23:26:57.078553 | miwear | 0x400 | armv8-m/arm_busfault.c:113 |

后一次的非零回溯项：

```text
0x0c2034f4
0x0c1a0bc6
0x0c2034da
0x0c19b97a
0x1c028fb0
```

前四项尚无精确符号化结果，不能凭位置指定函数名。仓库现有精确固件证据 `Canopus/targets/xiaomi-band-10-pro-3.101.043/evidence/EVID-RESOURCE-XIP-THUNKS-1043-001.json` 记载对象 class allocator 位于 `0x1c028f80`，class initializer 位于 `0x1c029074`。回溯项位于 allocator 入口之后 `0x30`，是定位 UI 创建路径的线索；当前没有该固件完整镜像用于核对故障指令。

`data/offlinelog/tmp.log` 同时包含两次启动后的：

```text
Crash happened!!!
Crash happened from board_crashdump!!!
resetcause=rtc_watchdog(1)
```

23:23 和 23:27 附近 bl2/AP 各有记录，不能将同一次启动的两份输出算成两次独立崩溃。较早的 `system` BusFault 与后一次 `miwear` BusFault 回溯不同，尚不能认定为同一个原因。

23:27:33–34 附近的全线程 `sched_dumpstack` 属于重启后的采集阶段，不作为打开 Loop 时的故障栈。

## 源码核对

- Loop 与 Lyra 的页面 `on_create(page, root, start_data)` 签名一致。
- Band 10 Pro 暂停与 UI 销毁槽的注册方式与 Lyra 一致。
- Loop 通过 Supervisor Launcher 服务注册页面；上游会复制临时页面指针数组，暂未发现该数组离开 Rust 栈后悬空的问题。
- 容器及字体样式使用与 Lyra 相同的 SDK 创建路径。相同写法仍需要真实设备验证，不能由此排除共用接口问题。
- Band 10 Pro 的私有滚动条模式字节写入与 Lyra 相同。日志没有证明它已经执行，更没有证明它是原因。

下一步需要启动阶段记录，区分进入页面回调、首个容器、标签、滚动容器、首次课表刷新和定时器创建，并记录关键对象指针及返回值；或补全故障寄存器和精确固件反汇编。应避免在证据不足时修改 ABI、固件符号地址或 verifier 白名单。

## WSL 分析与二进制保留

所有解压、二进制事件解析及 ELF 符号提取均在 Ubuntu-24.04 中完成。外层归档只提取普通文件，移除路径开头的 `/`，拒绝 `..`，不提取链接；再解压内层 `.gz`。二进制 note 按首字节记录长度遍历，仅对包含断言路径的记录解析文件名、行号、任务名和回溯，不将尾部未知字段猜作寄存器。

```sh
cd /mnt/d/Canopus/class-loop
llvm-nm-18 --numeric-sort --print-size --demangle \
  build/xiaomi-band-10-pro-3.101.043/loop.elf \
  > build/device-logs/29598/loop-symbols.txt
llvm-nm-18 --numeric-sort --print-size --demangle \
  ../Canopus/watchfaces/canopus-installer/build/xiaomi-band-10-pro-3.101.043/canopus_supervisor.elf \
  > build/device-logs/29598/supervisor-symbols.txt
```

已将当前本地二进制复制到 `build/device-logs/29598/binaries/`，便于后续构建后继续比对。尚无设备已安装文件哈希，以下仅标识本地文件。

| 文件 | SHA-256 |
|---|---|
| loop.elf（.043） | 2589c53aaa5e033deee41f984f637110ed3f7bf567639407a1e11721da74d8f6 |
| loop-0.3.0.canopus | d552cde2fea49347fa09f2e120673cad2c2dc2230b9751a09f65ccc39fee26ff |
| canopus_supervisor.elf（.043） | 28f14f6bac78fb06c1f2ea04fe32e046f400cbfafc07c10a4cd7f8fcccf0e3bb |
