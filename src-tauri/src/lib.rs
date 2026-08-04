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
    AvailableVersionGroup, InstallRequest, InstallResult, ManageInfo, RuntimesPayload,
    RuntimeKind, RuntimeVersion, ServiceInfo, ServiceInstallRequest, ServiceKind,
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
    activation::activate(&version)
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
    tauri::async_runtime::spawn_blocking(move || installer::install(&app, &request))
        .await
        .map_err(|e| format!("安装任务异常: {e}"))?
}

/// 卸载 NovaEnv 管理的版本。
#[tauri::command]
fn uninstall_version(version: RuntimeVersion) -> Result<(), String> {
    installer::uninstall(&version)
}

/// 获取管理目录信息（路径 / 版本数 / 占用空间）。
#[tauri::command]
fn get_manage_info() -> ManageInfo {    installer::manage_info()
}

// ---------- 服务类组件（Redis 等） ----------

/// 全部服务组件状态（安装情况 / 运行状态 / 端口）。
#[tauri::command]
fn list_services() -> Vec<ServiceInfo> {
    services::list_all()
}

/// 服务的可安装版本列表（按大版本分组，最新在前）。
#[tauri::command]
fn available_service_versions(
    kind: ServiceKind,
) -> Result<Vec<AvailableVersionGroup>, String> {
    match kind {
        ServiceKind::Redis => services::redis::available_version_groups(),
    }
}

/// 安装服务（异步执行，进度经 `service-progress` 事件推送）。
#[tauri::command]
async fn install_service(
    app: tauri::AppHandle,
    request: ServiceInstallRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            services::redis::install(&app, request.kind, &request.version)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (&app, request);
            Err("当前平台暂不支持 Redis 安装".to_string())
        }
    })
    .await
    .map_err(|e| format!("安装任务异常: {e}"))?
}

/// 卸载服务（保留数据目录）。
#[tauri::command]
fn uninstall_service(kind: ServiceKind, version: String) -> Result<(), String> {
    match kind {
        ServiceKind::Redis => {
            #[cfg(target_os = "macos")]
            {
                services::redis::uninstall(&version)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("当前平台暂不支持 Redis 卸载".to_string())
            }
        }
    }
}

/// 启动服务。
#[tauri::command]
fn start_service(kind: ServiceKind, version: String) -> Result<(), String> {
    match kind {
        ServiceKind::Redis => {
            #[cfg(target_os = "macos")]
            {
                services::redis::start(&version)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("当前平台暂不支持 Redis".to_string())
            }
        }
    }
}

/// 停止服务。
#[tauri::command]
fn stop_service(kind: ServiceKind, version: String) -> Result<(), String> {
    match kind {
        ServiceKind::Redis => {
            #[cfg(target_os = "macos")]
            {
                services::redis::stop(&version)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("当前平台暂不支持 Redis".to_string())
            }
        }
    }
}

/// 重启服务。
#[tauri::command]
fn restart_service(kind: ServiceKind, version: String) -> Result<(), String> {
    match kind {
        ServiceKind::Redis => {
            #[cfg(target_os = "macos")]
            {
                services::redis::restart(&version)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err("当前平台暂不支持 Redis".to_string())
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            uninstall_service,
            start_service,
            stop_service,
            restart_service
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
