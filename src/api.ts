import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActivationPreview,
  AvailableVersionGroup,
  InstallProgress,
  InstallResult,
  ManageInfo,
  RuntimesPayload,
  RuntimeKind,
  RuntimeVersion,
  ServiceInfo,
  VisionInfo,
  ServiceKind,
  ServiceProgress,
  ServiceConfig,
} from "./types";

/** 扫描全部运行时：概览 + 版本列表 */
export function listRuntimes(): Promise<RuntimesPayload> {
  return invoke("list_runtimes");
}

/** 生成切换默认版本的变更预览（不写入） */
export function previewActivation(
  version: RuntimeVersion,
): Promise<ActivationPreview> {
  return invoke("preview_activation", { version });
}

/** 执行切换默认版本 */
export function activate(version: RuntimeVersion): Promise<void> {
  return invoke("activate", { version });
}

/** 获取官方源可安装版本列表（按大版本分组；refresh=true 绕过缓存强制拉取） */
export function availableVersions(
  kind: RuntimeKind,
  refresh = false,
): Promise<AvailableVersionGroup[]> {
  return invoke("available_versions", { kind, refresh });
}

/** 安装指定版本（进度通过 install-progress 事件推送） */
export function installVersion(
  kind: RuntimeKind,
  version: string,
): Promise<InstallResult> {
  return invoke("install_version", { request: { kind, version } });
}

/** 卸载 NovaEnv 管理的版本 */
export function uninstallVersion(version: RuntimeVersion): Promise<void> {
  return invoke("uninstall_version", { version });
}

/** 监听安装进度事件，返回取消监听函数 */
export function onInstallProgress(
  handler: (progress: InstallProgress) => void,
): Promise<UnlistenFn> {
  return listen<InstallProgress>("install-progress", (event) =>
    handler(event.payload),
  );
}

/** 获取管理目录信息（路径 / 版本数 / 占用空间） */
export function getManageInfo(): Promise<ManageInfo> {
  return invoke("get_manage_info");
}

// ---------- 服务类组件 ----------

/** 全部服务组件状态 */
export function listServices(): Promise<ServiceInfo[]> {
  return invoke("list_services");
}

/** 服务的可安装版本（按大版本分组，最新在前） */
export function availableServiceVersions(
  kind: ServiceKind,
): Promise<AvailableVersionGroup[]> {
  return invoke("available_service_versions", { kind });
}

/** 安装服务（进度经 service-progress 事件推送；支持端口/密码配置） */
export function installService(
  kind: ServiceKind,
  version: string,
  config?: { port?: number; password?: string },
): Promise<void> {
  return invoke("install_service", {
    request: { kind, version, port: config?.port ?? null, password: config?.password ?? null },
  });
}

/** 修改服务运行配置（端口/密码）；运行中自动重启生效 */
export function updateServiceConfig(
  kind: ServiceKind,
  version: string,
  config: ServiceConfig,
): Promise<void> {
  return invoke("update_service_config", { kind, version, config });
}

/** 卸载服务（保留数据目录） */
export function uninstallService(
  kind: ServiceKind,
  version: string,
): Promise<void> {
  return invoke("uninstall_service", { kind, version });
}

/** 启动服务 */
export function startService(
  kind: ServiceKind,
  version: string,
): Promise<void> {
  return invoke("start_service", { kind, version });
}

/** 停止服务 */
export function stopService(
  kind: ServiceKind,
  version: string,
): Promise<void> {
  return invoke("stop_service", { kind, version });
}

/** 重启服务 */
export function restartService(
  kind: ServiceKind,
  version: string,
): Promise<void> {
  return invoke("restart_service", { kind, version });
}

/** 设置/取消服务开机自启（launchd：开机自启 + 崩溃自动拉起） */
export function setServiceAutostart(
  kind: ServiceKind,
  version: string,
  enabled: boolean,
): Promise<void> {
  return invoke("set_service_autostart", { kind, version, enabled });
}

/** 读取服务日志尾部（默认 200 行） */
export function serviceLogs(
  kind: ServiceKind,
  version: string,
  lines?: number,
): Promise<string> {
  return invoke("service_logs", { kind, version, lines: lines ?? 200 });
}

/** 监听服务安装进度事件，返回取消监听函数 */
export function onServiceProgress(
  handler: (progress: ServiceProgress) => void,
): Promise<UnlistenFn> {
  return listen<ServiceProgress>("service-progress", (event) =>
    handler(event.payload),
  );
}

/** 应用版本号 */
export async function getAppVersion(): Promise<string> {
  return invoke<string>("app_version");
}

/** 用系统默认浏览器打开链接 */
export async function openExternal(url: string): Promise<void> {
  await invoke("open_url", { url });
}

// ---------- Vision MCP 服务 ----------

export function visionStatus(): Promise<VisionInfo> {
  return invoke("vision_status");
}

export function visionStart(apiKey?: string): Promise<void> {
  return invoke("vision_start", { apiKey });
}

export function visionStop(): Promise<void> {
  return invoke("vision_stop");
}

export function visionLogs(): Promise<string> {
  return invoke("vision_logs");
}
