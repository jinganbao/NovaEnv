/**
 * 应用配置持久化（参考 NovaMsg 同款机制）
 * 配置自动保存到 localStorage，重启应用不丢失。
 */
import { reactive, watch } from "vue";

export type ThemeMode = "system" | "dark" | "light";

export interface AppConfig {
  themeMode: ThemeMode;
  themeAccent: string;
  autoCheckUpdate: boolean;
}

const STORAGE_KEY = "NovaEnv-config";

const defaults: AppConfig = {
  themeMode: "dark",
  themeAccent: "#34D399",
  autoCheckUpdate: true,
};

function loadConfig(): AppConfig {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      return { ...defaults, ...parsed };
    }
  } catch {
    // localStorage 数据损坏时回退到默认值
  }
  return { ...defaults };
}

const config = reactive<AppConfig>(loadConfig());

watch(
  () => ({ ...config }),
  (val) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(val));
  },
  { deep: true },
);

/** 应用配置（全局单例） */
export function useConfig() {
  return config;
}
