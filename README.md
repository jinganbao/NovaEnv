# NovaEnv

> 本地开发环境管理工具 — 一站式安装、切换与管理开发运行时与服务。

NovaEnv 是一个基于 Tauri 2 的桌面应用，帮助你像管理包依赖一样管理本机的开发环境：
安装指定版本、按大版本分组浏览、一键切换默认版本、服务化运行数据库与缓存。

## 功能

### 语言运行时

| 运行时 | 版本源 | 说明 |
| --- | --- | --- |
| ☕ Java (JDK) | Azul Zulu | 全版本安装，自动识别大版本与 LTS |
| 🟢 Node.js | nodejs.org 官方源 | 自动区分 LTS / Current |
| 🐹 Go | go.dev 官方源 | 大版本按 `1.x` 分组 |
| 📦 Maven | 国内镜像 + Apache 官方 | 完整历史版本 |
| 🐍 Python | python-build-standalone | 预编译 install_only 包，开箱即用 |

- 按大版本分组展示全部可用版本，已安装版本在列表中直接标注
- 同一大版本只保留最新小版本：安装新版自动移除旧版（旧版为默认时自动提升新版）
- 一键切换默认版本，自动写入 `~/.zshrc`（macOS）并 `source` 生效
- 支持卸载（仅限 NovaEnv 管理的版本）

### 服务类组件（macOS）

| 服务 | 能力 |
| --- | --- |
| 🗄️ Redis | 源码编译安装、自定义端口/密码、开机自启、崩溃自动拉起 |
| 🐬 MySQL | 官方预编译包、自定义端口/密码、开机自启、崩溃自动拉起 |

- 独立的进程管理（pid + 端口探测），应用退出后服务继续运行
- 数据目录与程序目录分离（`~/.novaenv/data`），卸载服务保留数据
- 服务日志实时查看

### 其他

- 🎨 主题系统：浅色 / 深色 / 跟随系统 + 多套配色预设
- 🔄 应用内更新检查与安装（GitHub Releases 通道）
- 📁 管理目录总览（各目录占用空间、服务数据）
- 📝 操作日志：`~/.novaenv/logs/app.log`
- ⚠️ 全局错误捕获：错误遮罩 + 一键重载，杜绝白屏

## 安装

### 从 Release 下载

前往 [Releases](https://github.com/jinganbao/NovaEnv/releases) 下载对应平台的安装包：

- macOS：`.dmg` / `.app.tar.gz`（Apple Silicon 与 Intel）
- Windows：`.msi` / `.exe`
- Linux：`.deb` / `.AppImage`

### 从源码构建

```bash
# 环境要求：Node.js ≥ 20、Rust stable、pnpm
pnpm install
pnpm tauri dev      # 开发模式
pnpm tauri build    # 打包
```

## 快速上手

1. 启动 NovaEnv，左侧选择运行时（如 Java）
2. 右侧按大版本列出全部可用版本，点击「安装」并等待进度完成
3. 安装后点击「设为默认」，新打开的终端即生效
4. 服务类组件：安装 Redis / MySQL 后可配置端口与密码，支持开机自启

### 目录结构

```
~/.novaenv/
├── installs/     # 运行时安装目录（java / node / go / maven / python）
├── services/     # 服务安装目录（redis / mysql，按版本）
├── data/         # 服务数据（卸载服务时保留）
├── logs/         # 应用日志与服务日志
└── run/          # pid / socket 文件
```

## 技术栈

- [Tauri 2](https://tauri.app/) + Rust（tokio、ureq、tar、zip）
- Vue 3 + TypeScript + Vite
- macOS：launchd（服务自启）、`libc`（进程管理）
- 更新：tauri-plugin-updater（GitHub Releases）

## 开发

```bash
cargo test        # Rust 单元测试
cargo clippy      # 静态检查（CI 门禁）
pnpm run build    # 前端类型检查 + 构建
```

发布流程（`.github/workflows/release.yml`）：打 tag 即触发四平台打包，
自动上传 GitHub Releases，应用内更新通道同步生效。

## License

[MIT](./LICENSE) © 2026 NovaHub
