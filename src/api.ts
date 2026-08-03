import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActivationPreview,
  AvailableVersionGroup,
  InstallProgress,
  RuntimesPayload,
  RuntimeKind,
  RuntimeVersion,
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
): Promise<void> {
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
