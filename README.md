# NovaEnv

> 开发环境可视化管理工具（ServBay 类）。管理本机已安装的 **JDK / Node.js / Go**，支持扫描展示、一键切换默认版本、按大版本安装/升级/卸载；架构预留下载安装引擎与多版本数据库（MySQL / PostgreSQL / Redis 等）服务管理。

[![GitHub release](https://img.shields.io/github/v/release/jinganbao/NovaEnv)](https://github.com/jinganbao/NovaEnv/releases)
[![GitHub Actions](https://img.shields.io/github/actions/workflow/status/jinganbao/NovaEnv/release.yml?label=Release%20CI)](https://github.com/jinganbao/NovaEnv/actions)
[![Tauri](https://img.shields.io/badge/Tauri-2-34d399)](https://v2.tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3-42b883)](https://vuejs.org)

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite
- **后端**: Rust + Tauri 2
- **平台**: macOS / Windows（跨平台，条件编译）

## 目录结构

```
NovaEnv/
├── .github/workflows/        # GitHub Actions（release.yml 四平台打包）
├── scripts/
│   ├── release.sh            # 发布脚本（版本更新 → 提交 → tag → 推送）
│   └── gen_icons.py          # 备用图标生成脚本（正式图标由 tauri icon 生成）
├── src/                      # 前端（Vue 3）
│   ├── api.ts                # invoke 封装（list_runtimes / preview_activation / activate）
│   ├── types.ts              # TS 类型（与 Rust models.rs 契约对齐）
│   ├── App.vue               # 主界面：侧边栏 + 概览条 + 环境详情
│   └── components/
│       ├── Sidebar.vue           # 左侧环境导航
│       ├── RuntimeDetail.vue     # 环境详情（当前默认 + 版本列表）
│       ├── RuntimeVersionList.vue# 按大版本分组的可用版本列表（安装/升级/切换/卸载）
│       ├── VersionCard.vue       # 版本卡片（已弃用，保留参考）
│       ├── ActivationModal.vue   # 切换默认确认弹窗
│       └── UninstallModal.vue    # 卸载确认弹窗
├── src-tauri/                # 后端（Rust + Tauri 2）
│   ├── src/
│   │   ├── models.rs           # 数据模型（RuntimeKind / RuntimeVersion 等）
│   │   ├── adapter.rs          # RuntimeAdapter trait（运行时抽象）
│   │   ├── activation.rs       # 切换默认环境引擎（预览 + 执行）
│   │   ├── installer.rs        # 安装/卸载引擎（官方源版本列表 + 下载 + 解压 + 缓存）
│   │   ├── platform/           # 平台实现（macos.rs / windows.rs，#[cfg] 分离）
│   │   ├── runtimes/           # 运行时适配器（java.rs / node.rs / go.rs）
│   │   ├── lib.rs              # Tauri 入口与 commands 注册
│   │   └── main.rs
│   ├── capabilities/           # Tauri 2 权限配置
│   └── icons/                  # 全套图标（app-icon.svg 源文件 + tauri icon 生成）
└── README.md
```

## 环境要求

- **Rust** (stable) — https://rustup.rs
- **Node.js** ≥ 18 — https://nodejs.org
- Tauri 2 系统依赖（macOS 需 Xcode Command Line Tools；Windows 需 WebView2 + MSVC Build Tools），详见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

## 常用命令

npm 工作流：

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发模式（前端 HMR + Rust 热编译）
npm run build        # 前端类型检查（vue-tsc）+ 构建
cargo check          # Rust 编译检查（在 src-tauri 目录）
npm run tauri build  # 打包发布（生成 .dmg / .msi 等）
```

pnpm 工作流（与 npm 等价，推荐）：

```bash
pnpm install         # 安装依赖（esbuild / @tauri-apps/cli 的构建脚本自动放行）
pnpm run tauri dev
pnpm run build
pnpm tauri build
```

> **pnpm 用户注意事项**：pnpm 10+ 默认阻止依赖的 postinstall 脚本。本项目已在
> `pnpm-workspace.yaml` 中配置 `allowBuilds` 放行 `esbuild` 与 `@tauri-apps/cli`，
> 并设置 `confirmModulesPurge: false` 避免非 TTY 环境（IDE 终端）下
> `ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY` 报错。

## 功能说明

| 功能 | 说明 |
| --- | --- |
| 扫描已安装运行时 | JDK（macOS `java_home -V` / Windows 注册表 + 常见目录）、Node（nvm / fnm / Homebrew / 官方）、Go（官方 / Homebrew / goenv） |
| 展示 | 左侧边栏导航 + 顶部当前版本概览；版本卡片显示版本号 / 发行版 / 路径 / 默认标记 / NovaEnv 管理标记 |
| 切换默认版本 | macOS 幂等更新 `~/.zshrc`（NovaEnv 管理块 + 自动备份 `~/.zshrc.novaenv.bak`）；Windows 通过 PowerShell 写入用户级环境变量（规避 setx PATH 截断） |
| 变更预览 | 切换前展示将写入的配置行与备份路径，确认后执行 |
| **安装新版本** | 右侧按大版本分组的可用版本列表（官方源：Adoptium Temurin / nodejs.org / go.dev），LTS 标记；已安装版本直接在对应大版本行后标注（含默认/卸载操作），同一大版本有小版本更新时显示「升级到 x.y.z」按钮；流式下载 + 进度事件 + 解压到 `~/.novaenv/installs/<kind>/<version>/` |
| **卸载** | 仅可卸载 NovaEnv 管理安装的版本（删除 `~/.novaenv/installs` 下目录），默认版本需先切换 |
| **设置** | 左侧边栏 ⚙️ 入口：主题（跟随系统/暗色/亮色 + Nova 系列 6 色主题预设）、版本更新（检查/下载/安装/自动检查开关）、管理目录（路径/版本数/占用空间统计） |

> 注意：切换后需重新打开终端（或 `source ~/.zshrc`）生效。
> 安装下载可能较大（JDK ~200MB / Node ~25MB / Go ~70MB），首次安装请耐心等待。

## 架构与扩展指南

核心抽象是 `RuntimeAdapter` trait（`src-tauri/src/adapter.rs`）：

```rust
pub trait RuntimeAdapter: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    fn scan(&self) -> Vec<RuntimeVersion>;        // 扫描已安装版本
    fn active_version(&self) -> Option<String>;   // 检测当前生效版本
}
```

**新增一种语言运行时（如 Python）只需 4 步：**

1. `models.rs` 的 `RuntimeKind` 增加枚举值（含 `display_name` / `env_var_name`）
2. 新建 `runtimes/python.rs` 实现 `RuntimeAdapter`
3. `runtimes/mod.rs` 的 `all()` 注册；`overview()` 补充概览字段
4. `activation.rs` 的 `shell_lines()` 补充对应环境变量行

前端无需改动（分组按 `RuntimeKind` 自动渲染），切换引擎自动适配（macOS/Windows 均已支持任意新运行时）。

## 后期规划

- [ ] **下载安装引擎**：从官方源（Adoptium / nodejs.org / go.dev）搜索、下载、解压安装新版本到应用管理目录
- [ ] **数据库服务管理**：MySQL / PostgreSQL / Redis 多版本安装、启动 / 停止 / 状态监控（可复用 `RuntimeAdapter` 抽象扩展为 Service 管理）
- [ ] 卸载已安装版本、版本间依赖切换回滚
- [ ] 正式图标（`npm run tauri icon <源图>`）与应用签名

## 发布流程

参考 NovaMsg 的整套发布体系（GitHub Actions 四平台打包 + Tauri updater 自动更新）：

```bash
# 1. 发布新版本（自动更新三处版本号 + 提交 + 打 tag + 推送）
./scripts/release.sh 0.1.1

# 2. 推送后 GitHub Actions 自动执行：
#    - macOS (Apple Silicon + Intel) / Ubuntu / Windows 四平台打包
#    - 生成安装包并发布 GitHub Release
#    - 生成 latest.json（updater 自动更新清单）
```

### 首次发布前配置（一次性）

1. **GitHub Secrets**（Settings → Secrets and variables → Actions）：
   - `TAURI_SIGNING_PRIVATE_KEY`：updater 签名私钥内容（本地生成：`pnpm tauri signer generate --ci --password "" -w ~/.tauri/novaenv.key`，文件在 `~/.tauri/novaenv.key`，`cat` 后粘贴）
2. 发布完成后应用内可检测更新（updater 端点指向 GitHub Releases `latest.json`）

## 主题

NovaMsg 同款设计语言：CSS 变量体系（`--bg-app/--bg-sider/--bg-panel/--bg-input` + 双层边框），NovaEnv 品牌色 **#34D399**（emerald）。设置页支持：
- 外观模式：跟随系统 / 暗色 / 亮色（持久化到 localStorage，`data-theme` 属性切换）
- 主题色：Nova 系列 6 色预设（NovaEnv / NovaMsg / NovaDB / NovaFlow / NovaOps / NovaAI），动态注入品牌变量

图标源文件 `src-tauri/icons/app-icon.svg`，改图标后执行 `pnpm tauri icon src-tauri/icons/app-icon.svg` 重新生成全套。

## 当前进度

- [x] 项目骨架（前端 + 后端 + 图标 + 权限配置）
- [x] 运行时扫描引擎（JDK / Node / Go，macOS + Windows）
- [x] 切换默认环境引擎（预览 + 幂等写入 + 备份）
- [x] 前端界面（左侧边栏布局 + 概览条 + 版本卡片）
- [x] 安装引擎（官方源版本列表 + 流式下载进度 + 解压安装到 ~/.novaenv）
- [x] 卸载（仅 NovaEnv 管理的版本，默认版本保护）
- [x] 全链路验证：`pnpm install` ✅ `pnpm run build` ✅ `cargo check`（零警告）✅ `tauri dev` 启动 ✅ 官方源 API/URL/归档结构实测 ✅
- [ ] 下载完整性校验（SHA256 checksum）
- [ ] 数据库服务管理（MySQL / PostgreSQL / Redis）
