/**
 * 应用更新逻辑（参考 NovaMsg 同款）
 * - 手动/自动检查更新
 * - 下载进度展示
 * - 取消下载
 * - 下载完成后重启
 */
import { computed, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { checkAppUpdate, type UpdateResult } from "../utils/update";
import type { AppConfig } from "./useConfig";

export function useAppUpdate(
  config: AppConfig,
  feedback: { error: (m: string) => void; success: (m: string) => void },
) {
  const checkingUpdate = ref(false);
  const updateInfo = ref<UpdateResult | null>(null);
  const showUpdatePanel = ref(false);
  const installingUpdate = ref(false);
  const cancellingUpdate = ref(false);
  const updateDownloaded = ref(0);
  const updateTotal = ref(0);
  const currentVersion = ref("");
  const latestVersion = ref("");
  const updateError = ref<string | null>(null);

  const updateProgressPercentage = computed(() => {
    if (updateTotal.value > 0) {
      return Math.min(100, Math.round((updateDownloaded.value / updateTotal.value) * 100));
    }
    return 0;
  });

  const updateProgressLabel = computed(() => {
    if (updateTotal.value > 0) {
      return `正在下载更新… ${(updateDownloaded.value / 1024 / 1024).toFixed(1)} / ${(updateTotal.value / 1024 / 1024).toFixed(1)} MB`;
    }
    return `正在下载更新… ${(updateDownloaded.value / 1024 / 1024).toFixed(1)} MB`;
  });

  function errMsg(e: unknown): string {
    if (e instanceof Error) return e.message;
    return String(e ?? "未知错误");
  }

  async function loadCurrentVersion() {
    if (currentVersion.value) return;
    try {
      currentVersion.value = await getVersion();
    } catch {
      currentVersion.value = "";
    }
  }

  async function checkForUpdates(options?: { silent?: boolean }) {
    checkingUpdate.value = true;
    updateError.value = null;
    try {
      await loadCurrentVersion();
      const result = await checkAppUpdate();
      updateInfo.value = result;
      latestVersion.value = result.hasUpdate
        ? result.version ?? ""
        : result.currentVersion ?? currentVersion.value;

      if (!result.hasUpdate) {
        if (!options?.silent) {
          feedback.success("当前已是最新版本");
        }
        showUpdatePanel.value = false;
        return;
      }
      showUpdatePanel.value = true;
    } catch (e) {
      if (!options?.silent) {
        feedback.error(`检查更新失败: ${errMsg(e)}`);
      }
    } finally {
      checkingUpdate.value = false;
    }
  }

  async function handleUpdateDownload() {
    if (!updateInfo.value?.hasUpdate) return;
    installingUpdate.value = true;
    updateDownloaded.value = 0;
    updateTotal.value = 0;
    updateError.value = null;
    try {
      await updateInfo.value.downloadAndInstall((progress) => {
        if (progress.total > 0) updateTotal.value = progress.total;
        updateDownloaded.value = progress.downloaded;
      });
    } catch (e) {
      if (cancellingUpdate.value) {
        updateError.value = "已取消更新";
      } else {
        updateError.value = `安装更新失败: ${errMsg(e)}`;
      }
    } finally {
      installingUpdate.value = false;
      cancellingUpdate.value = false;
    }
  }

  function cancelUpdateDownload() {
    cancellingUpdate.value = true;
    updateInfo.value?.cancel();
  }

  function closeUpdatePanel() {
    showUpdatePanel.value = false;
    updateInfo.value = null;
  }

  /** 启动时自动检查更新（静默模式，仅 config.autoCheckUpdate 为 true 时执行） */
  async function autoCheckOnStartup() {
    if (!config.autoCheckUpdate) return;
    await checkForUpdates({ silent: true });
  }

  return {
    checkingUpdate,
    showUpdatePanel,
    updateInfo,
    installingUpdate,
    cancellingUpdate,
    updateDownloaded,
    updateTotal,
    updateProgressPercentage,
    updateProgressLabel,
    currentVersion,
    latestVersion,
    updateError,
    loadCurrentVersion,
    checkForUpdates,
    handleUpdateDownload,
    cancelUpdateDownload,
    closeUpdatePanel,
    autoCheckOnStartup,
  };
}
