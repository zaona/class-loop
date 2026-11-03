# Loop

腕上手环课表应用，基于 [Canopus](../Canopus) 原生模块框架。Launcher 显示名为 **Loop**。

## 目录关系

本仓库必须与 Canopus 框架互为兄弟目录：

```
D:\Canopus\
  Canopus\                 # 框架 / SDK / target packs
  Canopus-App-LyraPlayer\  # 参考应用
  Canopus-Module-Corona\   # 系统组件 UI 参考
  class-loop\              # 本仓库
```

`loop-device` 通过相对路径依赖 `../../../Canopus/sdk/rust/...`。

当前构建使用 `ds-newdevice-supports@c5573a69`。此前的分支适配记录见 [新分支适配记录](docs/DS-NEWDEVICE-SUPPORTS.md)。

当前为 0.3.6 / versionCode 9：修复长课程信息撑高原生列表卡片的问题，沿用管理器列表行的固定高度与单行滚动；保留 v1 文案、按钮、五个原生页面和固件切页。无需更新管理器。逐项功能对照和 WSL 验证见 [v1 功能对齐记录](docs/V1-PARITY-034.md)。0.3.2 用户反馈打开不再重启，证据见 [第三轮设备日志](docs/DEVICE-CRASH-29598-3.md)；0.3.6 尚待真机验证。

0.3.5 修复按住课程期间导入造成的点击错位，以及固件切页失败后的页面恢复，并移除无引用的旧构建入口、目标配置和重复图标。清理清单及本轮验证见 [审查修复记录](docs/CLEANUP-REVIEW.md)。

## 模块身份

| 项 | 值 | 说明 |
|---|---|---|
| module token | `canopus_loop` | 收据 / inbox 文件名及 CanopusConnect 通道 |
| display | `Loop` | Launcher 文案 |
| app_id | Supervisor 动态分配 | 五个模块原生页面与 Launcher |
| ABI / version | `2` / `0.3.6` (`versionCode = 9`) | 固定原生列表行高度，沿用当前 ABI 2 Supervisor |
| lifecycle | `resident-after-activation` | 注销需重启 |

## 一期范围

- 首页：现在 / 下一节 / 今日剩余
- 今日、本周、课程详情、数据管理（手动刷新 / 清空本地课表）
- 运行时课表：CanopusConnect 直接接收，保存至模块私有目录 `/data/canopus/m.canopus_loop/schedule.json`（无文件则空课表）
- AstroBox 插件：`plugins/astrobox-loop-import`（转换 ICS、分片推送并等待写入确认）
- 仅支持提供 CanopusConnect 的三个固件目标
- 目标：Band 10 Pro `3.101.043`，Band 11 `4.100.155` + `4.100.179`

不做：腕上编辑、上课提醒、固件内置课表。

## 仓库结构

```
crates/loop-core/                 no_std 课表模型与 view 展示数据
crates/loop-device/               设备 staticlib（模块描述符 / launcher / 日期）
crates/loop-device/src/target/ui/  系统预制组件、原生页面生命周期与事件
crates/loop-protocol/             手机与设备共享的 Connect 分片协议
tools/loop-ui-host/               实际原生页面与组件源码的主机回归
plugins/astrobox-loop-import/     AstroBox 课表导入插件
scripts/                          WSL 构建、打包与图标编码
crates/loop-device/assets/        Launcher 图标 PNG 源与内嵌 LVGL BIN
dist/                            官方 CLI 生成的包与设备族安装器（忽略）
docs/BUILD.md                     WSL 构建说明
docs/SCHEDULE_IMPORT_PROTOCOL.md  插件↔原生模块 Connect 协议
```

## 快速构建

完整的环境搭建、日常开发、验证与排错见 [WSL 开发流程](docs/WSL-DEVELOPMENT.md)；与参考应用的关系见 [Lyra ng 结构对照](docs/LYRA-NG-COMPARISON.md)。

详见 [docs/BUILD.md](docs/BUILD.md)。在 WSL Ubuntu 中：

```sh
cd /mnt/d/Canopus/class-loop
sh scripts/build-wsl.sh
# 也可只构建一个设备族或精确固件：
sh scripts/build-wsl.sh xiaomi-band-11
sh scripts/build-wsl.sh xiaomi-band-10-pro-3.101.043
```

不指定设备时构建全部三个目标，安装器按 10 Pro（`.043`）与 Band 11（`.155`、`.179`）分目录。
产物为 `dist/loop-0.3.6.canopus` 与 `dist/installer/<设备族>/`；部分目标的包名带固件版本后缀。旁边的 `.build-info.json` 记录 SDK 分支/commit、CLI 与包及各 ELF 的 SHA-256。
当前只保留完整三目标包及对应安装器；`.043` 设备直接使用完整包，不另保留专用包。日常构建使用不带参数的 `sh scripts/build-wsl.sh`。
安装器由 `canopus package --installer` 生成，使用 CMI2 回执，不再需要签名私钥。需要匹配固件的 ABI 2 Supervisor；已经能运行 0.3.3 的管理器无需为本次更新重装。
Launcher 图标随模块内嵌，发布应用前写入 `/data/canopus/m.canopus_loop/appicon_loop.bin`。
标题栏、列表行、字体与滚动使用管理器同类系统预制组件，由 Loop 原生页面适配层通过已有精确 target SDK 创建；不绘制自制圆角卡片或返回键、不写私有滚动条字段。课程连续滚动，移除上一版七项分页。文案和点击含义按 v1 恢复。

当前页面使用 UI 线程定时器刷新；暂停时删除定时器，恢复时重新创建，销毁时释放页面状态。五个页面走固件 push/pop，与 Lyra 一样使用原生生命周期，沿用 Supervisor 动态 ID。旧自制像素布局与预览脚本未恢复；历史见 [UI 重写记录](docs/UI-REWRITE.md)。

## 课表数据

见 [docs/SCHEDULE.md](docs/SCHEDULE.md)。设备不解析 ICS；由 AstroBox 插件转换后推送。

手机推送：启用原生 **Loop**，在 AstroBox 安装 `plugins/astrobox-loop-import` 构建出的 `.abp`，选择 ICS 来源（WakeUp / WeekDown / Nexio）后导入本地 `.ics` 推送。无需安装额外手表快应用。已有旧沙箱课表请通过插件重新推送。协议见 [docs/SCHEDULE_IMPORT_PROTOCOL.md](docs/SCHEDULE_IMPORT_PROTOCOL.md)。

```sh
cd plugins/astrobox-loop-import
rustup target add wasm32-wasip2
python3 scripts/build_dist.py --release --package
```

## License

AGPL-3.0（与 Canopus 一致）
