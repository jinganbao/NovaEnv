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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_log("app", &format!("NovaEnv 启动 v{}", env!("CARGO_PKG_VERSION")));
    tauri::Builder::default()
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
            service_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
