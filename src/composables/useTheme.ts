/**
 * 主题逻辑（参考 NovaMsg 同款：主题色预设 + CSS 变量注入）
 * - 明暗模式：通过 html[data-theme] 属性控制（styles.css 中定义）
 * - 主题色：动态计算 brand 系列变量注入 documentElement
 */
import { computed, watch } from "vue";
import type { AppConfig, ThemeMode } from "./useConfig";

/** Nova 系列主题色预设 */
export const themePresets = [
  { name: "NovaEnv", color: "#34D399" },
  { name: "NovaMsg", color: "#3DD6C6" },
  { name: "NovaDB", color: "#5BA8FF" },
  { name: "NovaFlow", color: "#A3E635" },
  { name: "NovaOps", color: "#F59E0B" },
  { name: "NovaAI", color: "#8BDAFF" },
];

export const themeModeOptions: { label: string; value: ThemeMode }[] = [
  { label: "跟随系统", value: "system" },
  { label: "暗色", value: "dark" },
  { label: "亮色", value: "light" },
];

function hexToRgb(hex: string) {
  const normalized = hex.replace("#", "");
  const value =
    normalized.length === 3
      ? normalized
          .split("")
          .map((char) => char + char)
          .join("")
      : normalized;
  const num = Number.parseInt(value, 16);
  if (Number.isNaN(num)) return { r: 52, g: 211, b: 153 };
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255,
  };
}

function mix(hex: string, target: string, weight: number) {
  const a = hexToRgb(hex);
  const b = hexToRgb(target);
  const channel = (x: number, y: number) =>
    Math.round(x * (1 - weight) + y * weight);
  return `#${[channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b)]
    .map((part) => part.toString(16).padStart(2, "0"))
    .join("")}`;
}

function rgba(hex: string, alpha: number) {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function useTheme(config: AppConfig) {
  /** 设置明暗模式：system 时移除属性（跟随系统），否则显式指定 */
  function setThemeMode(mode: ThemeMode) {
    config.themeMode = mode;
  }

  /** 应用 data-theme 属性（决定 styles.css 中的明暗变量） */
  function applyModeAttribute(mode: ThemeMode) {
    const html = document.documentElement;
    if (mode === "system") {
      html.removeAttribute("data-theme");
    } else {
      html.setAttribute("data-theme", mode);
    }
  }

  /** 品牌色系列变量（覆盖 styles.css 中的 :root 定义） */
  const brandVars = computed(() => {
    const accent = config.themeAccent || "#34D399";
    const dark = (config.themeMode === "system" && !window.matchMedia("(prefers-color-scheme: light)").matches) || config.themeMode === "dark";
    return {
      "--brand": accent,
      "--brand-hover": mix(accent, "#FFFFFF", 0.18),
      "--brand-active": mix(accent, "#000000", 0.18),
      "--brand-soft": rgba(accent, dark ? 0.14 : 0.12),
    };
  });

  watch(
    () => config.themeMode,
    (mode) => applyModeAttribute(mode),
    { immediate: true },
  );

  watch(brandVars, (vars) => {
    for (const [key, value] of Object.entries(vars)) {
      document.documentElement.style.setProperty(key, value);
    }
  }, { immediate: true });

  return {
    themePresets,
    themeModeOptions,
    brandVars,
    setThemeMode,
  };
}
