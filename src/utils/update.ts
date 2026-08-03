/**
 * 应用更新封装（基于 @tauri-apps/plugin-updater）
 */
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface DownloadProgress {
  downloaded: number;
  total: number;
}

export interface UpdateResult {
  hasUpdate: boolean;
  version?: string;
  currentVersion?: string;
  date?: string;
  body?: string;
  /** 取消当前下载（会中断 downloadAndInstall） */
  cancel: () => void;
  downloadAndInstall: (
    onProgress?: (progress: DownloadProgress) => void,
  ) => Promise<void>;
}

export async function checkAppUpdate(): Promise<UpdateResult> {
  const update: Update | null = await check();

  if (!update) {
    return {
      hasUpdate: false,
      cancel: () => {},
      downloadAndInstall: async () => {},
    };
  }

  let cancelled = false;
  let accumulatedDownloaded = 0;

  return {
    hasUpdate: true,
    version: update.version,
    currentVersion: update.currentVersion,
    date: update.date,
    body: update.body,
    cancel: () => {
      cancelled = true;
      update.close();
    },
    downloadAndInstall: async (
      onProgress?: (progress: DownloadProgress) => void,
    ) => {
      accumulatedDownloaded = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          if (event.data.contentLength) {
            onProgress?.({ downloaded: 0, total: event.data.contentLength });
          }
        } else if (event.event === "Progress") {
          accumulatedDownloaded += event.data.chunkLength;
          onProgress?.({ downloaded: accumulatedDownloaded, total: 0 });
        }
      });
      if (cancelled) return;
      await relaunch();
    },
  };
}
