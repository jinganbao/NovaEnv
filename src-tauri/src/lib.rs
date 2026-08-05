// NovaEnv Tauri 应用入口

mod activation;
mod adapter;
mod installer;
mod models;
mod platform;
mod runtimes;
mod services;

use activation::ActivationPreview;
use models::{
    AvailableVersionGroup, InstallRequest, InstallResult, ManageInfo, RuntimeKind, RuntimeVersion,
    RuntimesPayload, ServiceConfig, ServiceInfo, ServiceInstallRequest, ServiceKind,
};

/// 扫描全部运行时（JDK / Node / Go），返回概览 + 完整版本列表。
#[tauri::command]
fn list_runtimes() -> Result<RuntimesPayload, String> {
    runtimes::scan_all()
}

/// 生成切换默认版本的变更预览（不写入）。
#[tauri::command]
fn preview_activation(version: RuntimeVersion) -> Result<ActivationPreview, String> {
    activation::preview(&version)
}

/// 执行切换默认版本（写入 shell 配置 / 用户环境变量）。
#[tauri::command]
fn activate(version: RuntimeVersion) -> Result<(), String> {
    let detail = format!("切换默认 {:?} {}", version.kind, version.version);
    let result = activation::activate(&version);
    log_result("activate", &detail, &result);
    result
}

/// 获取官方源的可安装版本列表（按大版本分组，带 5 分钟缓存）。
#[tauri::command]
fn available_versions(
    kind: RuntimeKind,
    refresh: Option<bool>,
) -> Result<Vec<AvailableVersionGroup>, String> {
    installer::available_versions(kind, refresh.unwrap_or(false))
}

/// 安装指定版本（异步执行，进度通过 `install-progress` 事件推送）。
#[tauri::command]
async fn install_version(
    app: tauri::AppHandle,
    request: InstallRequest,
) -> Result<InstallResult, String> {
    let detail = format!("安装 {:?} {}", request.kind, request.version);
    let result = tauri::async_runtime::spawn_blocking(move || installer::install(&app, &request))
        .await
        .map_err(|e| format!("安装任务异常: {e}"))?;
    app_log("install", &format!("{detail} → {}", if result.is_ok() { "成功" } else { "失败" }));
    result
}

/// 卸载 NovaEnv 管理的版本。
#[tauri::command]
fn uninstall_version(version: RuntimeVersion) -> Result<(), String> {
    let detail = format!("卸载 {:?} {}", version.kind, version.version);
    let result = installer::uninstall(&version);
    log_result("uninstall", &detail, &result);
    result
}

/// 应用版本号（与 Cargo.toml 一致，构建时注入）
#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 用系统默认浏览器/应用打开链接
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open_url_sys(&url)
}

fn open_url_sys(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .status()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    Ok(())
}

/// 获取管理目录信息（路径 / 版本数 / 占用空间）。
#[tauri::command]
fn get_manage_info() -> ManageInfo {
    installer::manage_info()
}

// ---------- 服务类组件（Redis 等） ----------

/// 全部服务组件状态（安装情况 / 运行状态 / 端口）。
#[tauri::command]
fn list_services() -> Vec<ServiceInfo> {
    services::list_all()
}

/// 服务的可安装版本列表（按大版本分组，最新在前）。
#[tauri::command]
fn available_service_versions(kind: ServiceKind) -> Result<Vec<AvailableVersionGroup>, String> {
    match kind {
        ServiceKind::Redis => services::redis::available_version_groups(),
        ServiceKind::MySql => services::mysql::available_version_groups(),
    }
}

/// 安装服务（异步执行，进度经 `service-progress` 事件推送；支持端口/密码配置）。
#[tauri::command]
async fn install_service(
    app: tauri::AppHandle,
    request: ServiceInstallRequest,
) -> Result<(), String> {
    let detail = format!("安装服务 {:?} {}", request.kind, request.version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match request.kind {
                ServiceKind::Redis => services::redis::install(
                    &app,
                    request.kind,
                    &request.version,
                    request.port,
                    request.password,
                ),
                ServiceKind::MySql => services::mysql::install(
                    &app,
                    request.kind,
                    &request.version,
                    request.port,
                    request.password,
                ),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (&app, request);
            Err("当前平台暂不支持服务安装".to_string())
        }
    })
    .await
    .map_err(|e| format!("安装任务异常: {e}"))?;
    app_log("service-install", &format!("{detail} → {}", if result.is_ok() { "成功" } else { "失败" }));
    result
}

/// 修改服务运行配置（端口 / 密码）；运行中自动重启生效。
#[tauri::command]
async fn update_service_config(
    kind: ServiceKind,
    version: String,
    config: ServiceConfig,
) -> Result<(), String> {
    let detail = format!("修改服务配置 {:?} {}", kind, version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::update_config(&version, &config),
                ServiceKind::MySql => services::mysql::update_config(&version, &config),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version, config);
            Err("当前平台暂不支持服务配置".to_string())
        }
    })
    .await
    .map_err(|e| format!("配置任务异常: {e}"))?;
    app_log("service-config", &format!("{detail} → {}", if result.is_ok() { "成功" } else { "失败" }));
    result
}

/// 卸载服务（保留数据目录）。
#[tauri::command]
async fn uninstall_service(kind: ServiceKind, version: String) -> Result<(), String> {
    let detail = format!("卸载服务 {:?} {}", kind, version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::uninstall(&version),
                ServiceKind::MySql => services::mysql::uninstall(&version),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version);
            Err("当前平台暂不支持服务卸载".to_string())
        }
    })
    .await
    .map_err(|e| format!("卸载任务异常: {e}"))?;
    app_log("service-uninstall", &format!("{detail} → {}", if result.is_ok() {"成功"} else {"失败"}));
    result
}

/// 启动服务（异步，不阻塞界面）。
#[tauri::command]
async fn start_service(kind: ServiceKind, version: String) -> Result<(), String> {
    let detail = format!("启动服务 {:?} {}", kind, version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::start(&version),
                ServiceKind::MySql => services::mysql::start(&version),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version);
            Err("当前平台暂不支持服务".to_string())
        }
    })
    .await
    .map_err(|e| format!("启动任务异常: {e}"))?;
    app_log("service-start", &format!("{detail} → {}", if result.is_ok() {"成功"} else {"失败"}));
    result
}

/// 停止服务（异步，不阻塞界面）。
#[tauri::command]
async fn stop_service(kind: ServiceKind, version: String) -> Result<(), String> {
    let detail = format!("停止服务 {:?} {}", kind, version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::stop(&version),
                ServiceKind::MySql => services::mysql::stop(&version),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version);
            Err("当前平台暂不支持服务".to_string())
        }
    })
    .await
    .map_err(|e| format!("停止任务异常: {e}"))?;
    app_log("service-stop", &format!("{detail} → {}", if result.is_ok() {"成功"} else {"失败"}));
    result
}

/// 重启服务（异步，不阻塞界面）。
#[tauri::command]
async fn restart_service(kind: ServiceKind, version: String) -> Result<(), String> {
    let detail = format!("重启服务 {:?} {}", kind, version);
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::restart(&version),
                ServiceKind::MySql => services::mysql::restart(&version),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version);
            Err("当前平台暂不支持服务".to_string())
        }
    })
    .await
    .map_err(|e| format!("重启任务异常: {e}"))?;
    app_log("service-restart", &format!("{detail} → {}", if result.is_ok() {"成功"} else {"失败"}));
    result
}

/// 设置/取消服务开机自启（launchd 托管：开机自启 + 崩溃自动拉起）。
#[tauri::command]
async fn set_service_autostart(
    kind: ServiceKind,
    version: String,
    enabled: bool,
) -> Result<(), String> {
    let detail = format!("设置开机自启 {:?} {} → {}", kind, version, if enabled { "开启" } else { "关闭" });
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::set_autostart(&version, enabled),
                ServiceKind::MySql => services::mysql::set_autostart(&version, enabled),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version, enabled);
            Err("当前平台暂不支持开机自启".to_string())
        }
    })
    .await
    .map_err(|e| format!("自启配置任务异常: {e}"))?;
    app_log("service-autostart", &format!("{detail} → {}", if result.is_ok() { "成功" } else { "失败" }));
    result
}

/// 读取服务日志尾部（默认 200 行）。
#[tauri::command]
async fn service_logs(
    kind: ServiceKind,
    version: String,
    lines: Option<usize>,
) -> Result<String, String> {
    let lines = lines.unwrap_or(200);
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            match kind {
                ServiceKind::Redis => services::redis::tail_log(&version, lines),
                ServiceKind::MySql => services::mysql::tail_log(&version, lines),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (kind, version, lines);
            Err("当前平台暂不支持服务日志".to_string())
        }
    })
    .await
    .map_err(|e| format!("日志任务异常: {e}"))?
}

// ---------- Vision MCP 服务（AI 视觉） ----------

/// Vision 服务状态
#[tauri::command]
fn vision_status() -> services::vision::VisionInfo {
    services::vision::info()
}

/// 启动 Vision MCP 服务（首次自动部署 + venv + 依赖安装）
#[tauri::command]
async fn vision_start(
    app: tauri::AppHandle,
    api_key: Option<String>,
) -> Result<(), String> {
    let source = resolve_vision_source(&app);
    tauri::async_runtime::spawn_blocking(move || {
        services::vision::start(&source, api_key)
    })
    .await
    .map_err(|e| format!("Vision 启动任务异常: {e}"))?
}

/// 停止 Vision MCP 服务
#[tauri::command]
async fn vision_stop() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(services::vision::stop)
        .await
        .map_err(|e| format!("Vision 停止任务异常: {e}"))?
}

/// Vision 服务日志尾部
#[tauri::command]
fn vision_logs() -> Result<String, String> {
    services::vision::logs()
}

/// 定位 mcp-vision 源目录：打包资源优先，dev 回退仓库目录
fn resolve_vision_source(app: &tauri::AppHandle) -> std::path::PathBuf {
    use tauri::Manager;
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("mcp-vision");
        if p.join("server.py").is_file() {
            return p;
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("mcp-vision")
}

// ---------- 应用日志 ----------

/// 应用日志：追加写入 ~/.novaenv/logs/app.log（带时间戳）。
/// 记录安装/卸载/切换/服务操作与错误，便于问题排查。
pub(crate) fn app_log(action: &str, detail: &str) {
    let dir = installer::novaenv_dir().join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let line = format!("{ts} [{action}] {detail}\n");
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("app.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// 记录 command 结果（成功/失败）
fn log_result(command: &str, detail: &str, result: &Result<(), String>) {
    match result {
        Ok(()) => app_log(command, &format!("{detail} → 成功")),
        Err(e) => app_log(command, &format!("{detail} → 失败: {e}")),
    }
}

// ---------- 启动自检 ----------

/// 启动自检：目录结构就绪 / 清理安装残留 / 配置完整性检查
fn self_check() {
    // 1) 标准目录结构（installs / services / data / logs / run）
    for dir in ["installs", "services", "data", "logs", "run"] {
        let p = installer::novaenv_dir().join(dir);
        if let Err(e) = std::fs::create_dir_all(&p) {
            app_log("self-check", &format!("创建目录失败 {dir}: {e}"));
        }
    }

    // 2) 清理安装残留：上次中断留下的解压目录 + 超过 24h 的半成品归档
    let downloads = installer::installs_dir().join("_downloads");
    if let Ok(entries) = std::fs::read_dir(&downloads) {
        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_extract_dir = path.is_dir()
                && path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with("extract-"))
                    .unwrap_or(false);
            if is_extract_dir {
                if std::fs::remove_dir_all(&path).is_ok() {
                    removed += 1;
                }
                continue;
            }
            let is_stale_archive = path
                .extension()
                .map(|e| e == "tar.gz" || e == "zip")
                .unwrap_or(false)
                && entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.elapsed()
                            .map(|d| d.as_secs() > 24 * 3600)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
            if is_stale_archive && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            app_log("self-check", &format!("清理安装残留 {removed} 项"));
        }
    }

    // 3) ~/.zshrc 管理块完整性检查
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = platform::home_dir() {
            if let Ok(content) = std::fs::read_to_string(home.join(".zshrc")) {
                let has_start = content.contains("# >>> NovaEnv managed >>>");
                let has_end = content.contains("# <<< NovaEnv managed <<<");
                if has_start && !has_end {
                    app_log("self-check", "警告：~/.zshrc 管理块缺少结束标记");
                }
            }
        }
    }
}

/// 构建中文系统菜单（macOS 顶部菜单栏 / Windows 菜单）
/// 关于面板走系统原生 About（版本号、版权、项目主页）
fn build_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    let app_name = "NovaEnv";
    let separator = PredefinedMenuItem::separator(app)?;

    // 应用菜单（关于 → 前端弹窗；检查更新紧随其后）
    let about = MenuItem::with_id(app, "menu-about", "关于 NovaEnv", true, None::<&str>)?;
    let check_update = MenuItem::with_id(app, "menu-update", "检查更新…", true, None::<&str>)?;
    let hide = PredefinedMenuItem::hide(app, Some("隐藏 NovaEnv"))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?;
    let show_all = PredefinedMenuItem::show_all(app, Some("全部显示"))?;
    let quit = MenuItem::with_id(app, "app-quit", "退出 NovaEnv", true, Some("Cmd+Q"))?;
    let app_menu = Submenu::with_items(
        app,
        app_name,
        true,
        &[
            &about,
            &check_update,
            &separator,
            &hide,
            &hide_others,
            &show_all,
            &separator,
            &quit,
        ],
    )?;

    // 编辑菜单（输入框快捷键需要）
    let undo = PredefinedMenuItem::undo(app, Some("撤销"))?;
    let redo = PredefinedMenuItem::redo(app, Some("重做"))?;
    let cut = PredefinedMenuItem::cut(app, Some("剪切"))?;
    let copy = PredefinedMenuItem::copy(app, Some("拷贝"))?;
    let paste = PredefinedMenuItem::paste(app, Some("粘贴"))?;
    let select_all = PredefinedMenuItem::select_all(app, Some("全选"))?;
    let edit_menu = Submenu::with_items(
        app,
        "编辑",
        true,
        &[&undo, &redo, &separator, &cut, &copy, &paste, &separator, &select_all],
    )?;

    // 视图菜单
    let fullscreen = PredefinedMenuItem::fullscreen(app, Some("进入全屏幕"))?;
    let view_menu = Submenu::with_items(app, "视图", true, &[&fullscreen])?;

    // 窗口菜单
    let minimize = PredefinedMenuItem::minimize(app, Some("最小化"))?;
    let close_window = PredefinedMenuItem::close_window(app, Some("关闭窗口"))?;
    let window_menu = Submenu::with_items(app, "窗口", true, &[&minimize, &close_window])?;

    // 帮助菜单
    let help_docs = MenuItem::with_id(app, "help-docs", "使用文档", true, None::<&str>)?;
    let help_homepage = MenuItem::with_id(app, "help-homepage", "项目主页", true, None::<&str>)?;
    let help_issues = MenuItem::with_id(app, "help-issues", "报告问题", true, None::<&str>)?;
    let help_license = MenuItem::with_id(app, "help-license", "开源许可证", true, None::<&str>)?;
    let help_menu = Submenu::with_items(
        app,
        "帮助",
        true,
        &[
            &help_docs,
            &separator,
            &help_homepage,
            &help_issues,
            &separator,
            &help_license,
        ],
    )?;

    let menu = Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &view_menu, &window_menu, &help_menu],
    )?;
    app.set_menu(menu)?;

    // 菜单事件：关于 → 前端自定义弹窗（含作者/网站）；检查更新 → 更新弹窗；帮助 → 文档/主页/反馈/许可证
    app.on_menu_event(|app, event| match event.id().as_ref() {
        "menu-about" => {
            use tauri::Emitter;
            let _ = app.emit("novaenv-about", ());
        }
        "app-quit" => {
            // 强制退出（Cmd+Q / 菜单退出），不驻留
            app.exit(0);
        }
        "menu-update" => {
            use tauri::Emitter;
            let _ = app.emit("novaenv-check-update", ());
        }
        "help-docs" => {
            let _ = open_url_sys("https://github.com/jinganbao/NovaEnv#readme");
        }
        "help-homepage" => {
            let _ = open_url_sys("https://github.com/jinganbao/NovaEnv");
        }
        "help-issues" => {
            let _ = open_url_sys("https://github.com/jinganbao/NovaEnv/issues");
        }
        "help-license" => {
            let _ = open_url_sys("https://github.com/jinganbao/NovaEnv/blob/main/LICENSE");
        }
        _ => {}
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_log("app", &format!("NovaEnv 启动 v{}", env!("CARGO_PKG_VERSION")));
    let app = tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            self_check();
            if let Err(e) = build_menu(app) {
                eprintln!("菜单构建失败: {e}");
            }
            // 关闭窗口 → 驻留 Dock（隐藏窗口，应用继续运行；Cmd+Q / 菜单退出才完全退出）
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }
            Ok(())
        })
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_runtimes,
            preview_activation,
            activate,
            available_versions,
            install_version,
            uninstall_version,
            get_manage_info,
            list_services,
            available_service_versions,
            install_service,
            update_service_config,
            uninstall_service,
            start_service,
            stop_service,
            restart_service,
            set_service_autostart,
            service_logs,
            vision_status,
            vision_start,
            vision_stop,
            vision_logs,
            app_version,
            open_url
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    // Dock 图标点击 → 恢复窗口（驻留模式）
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Reopen { .. } = event {
            use tauri::Manager;
            if let Some(w) = app_handle.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
    });
}
