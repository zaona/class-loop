# Loop

腕上手环课表应用，基于 [Canopus](../Canopus) 原生模块框架。Launcher 显示名为 **Loop**。

## 目录关系

本仓库必须与 Canopus 框架互为兄弟目录：

```
D:\Canopus\
  Canopus\                 # 框架 / SDK / target packs
  Canopus-App-LyraPlayer\  # 参考应用
  Canopus-Build\           # Windows 构建环境（env.sh / 签名密钥）
  class-loop\              # 本仓库
```

`loop-device` 通过相对路径依赖 `../../../Canopus/sdk/rust/...`。

## 模块身份

| 项 | 值 | 说明 |
|---|---|---|
| module token | `loop` | 收据 / inbox 文件名 |
| package | `com.canopus.loop` | 原生应用包名 |
| display | `Loop` | Launcher 文案 |
| app_id | `0x00CD` | 与 Lyra `0x00CC` 错开 |
| lifecycle | `resident-after-activation` | 注销需重启 |

## 一期范围

- 首页：现在 / 下一节 / 今日剩余
- 今日、本周、课程详情、数据管理（手动刷新 / 清空本地课表）
- 运行时课表：快应用 `top.zaona.loopimport` → `/data/files/top.zaona.loopimport/loop/schedule.json`（无文件则空课表）
- 快应用工程：`quickapps/loop-import`
- AstroBox 插件：`plugins/astrobox-loop-import`（推送 `schedule.json`）
- 目标：Band 10 Pro `3.101.036` + `3.101.043`

不做：腕上编辑、上课提醒、固件内置课表。

## 仓库结构

```
crates/loop-core/                 no_std 课表模型与 UI snapshot
crates/loop-device/               设备 staticlib（模块描述符 / launcher / LVGL）
quickapps/loop-import/            手表快应用（落盘 schedule.json）
plugins/astrobox-loop-import/     AstroBox 课表导入插件
scripts/                          ICS 转换与交叉编译
watchfaces/loop/                  单 target 安装表盘
watchfaces/loop-prod/             036+043 生产安装表盘
docs/BUILD.md                     Windows 构建说明
docs/SCHEDULE_IMPORT_PROTOCOL.md  插件↔快应用协议
```

## 快速构建

详见 [docs/BUILD.md](docs/BUILD.md)。摘要（Git Bash）：

```sh
. /d/Canopus/Canopus-Build/env.sh
/d/Canopus/class-loop/scripts/build-install-watchface.sh
/d/Canopus/class-loop/scripts/build-install-watchface-prod.sh
```

## 课表数据

见 [docs/SCHEDULE.md](docs/SCHEDULE.md)。主机转换（设备不解析 ICS）：

```sh
python scripts/ics-to-schedule.py path/to/课表.ics
python scripts/ics-to-schedule.py path/to/课表.ics --check
python scripts/ics-to-schedule.py path/to/课表.ics \
  --term-name 2026秋 --term-start 2026-08-31 \
  -o schedule.json
```

手机推送：打开手表 **Loop Import** 前台，在 AstroBox 安装 `plugins/astrobox-loop-import` 构建出的 `.abp`，选择 ICS 来源（WakeUp / WeekDown / Nexio）后导入本地 `.ics` 推送。协议见 [docs/SCHEDULE_IMPORT_PROTOCOL.md](docs/SCHEDULE_IMPORT_PROTOCOL.md)。

```sh
cd plugins/astrobox-loop-import
python scripts/build_dist.py --release --package
```

## License

AGPL-3.0（与 Canopus 一致）
