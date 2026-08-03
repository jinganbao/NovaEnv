// NovaEnv Tauri 应用入口

mod activation;
mod adapter;
mod installer;
mod models;
mod platform;
mod runtimes;

use activation::ActivationPreview;
use models::{
    AvailableVersionGroup, InstallRequest, ManageInfo, RuntimesPayload, RuntimeKind,
    RuntimeVersion,
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
) -> Result<(), String> {
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
fn get_manage_info() -> ManageInfo {
    installer::manage_info()
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
            get_manage_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
