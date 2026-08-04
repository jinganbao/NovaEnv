# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 约定。

## [1.0.0] - 2026-08-04

### 新增

- **语言运行时管理**：Java（Azul Zulu）、Node.js、Go、Maven、Python 五个运行时
  - 按大版本分组展示全部可用版本，已安装版本就地标注
  - 同大版本只保留最新小版本，安装新版自动移除旧版并提升默认
  - 一键切换默认版本（macOS 写入 `~/.zshrc` 并自动 `source`）
  - 下载进度条、安装/卸载/切换完整流程
- **服务类组件**（macOS）：Redis、MySQL
  - 自定义端口与密码，安装后可修改配置并自动重启生效
  - 开机自启（launchd）、崩溃自动拉起、服务日志实时查看
  - 数据目录与程序分离，卸载保留数据
- **设置中心**：主题系统（浅色/深色/跟随系统 + 配色预设）、版本更新检查与安装、管理目录总览、关于
- **工程质量**：应用日志（`~/.novaenv/logs/app.log`）、全局错误捕获遮罩、启动自检与残留清理、28+ 单元测试、CI 质量门禁（clippy + test）
- **发布就绪**：GitHub Actions 四平台打包、应用内更新通道、MIT License

### 修复

- Windows 构建：`winreg` 枚举键 API 兼容
- MySQL 安装：端口冲突检测（避免误连用户已有实例）、`my.cnf` 仅解析 `[mysqld]` 段
- Redis 版本解析：`.tar.gz` 尾点导致版本列表为空的问题
- 下载进度与文件大小统一以 MB 展示
