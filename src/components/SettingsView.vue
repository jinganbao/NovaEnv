<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getManageInfo } from "../api";
import { useConfig } from "../composables/useConfig";
import { themeModeOptions, themePresets, useTheme } from "../composables/useTheme";
import { RUNTIME_META } from "../types";
import type { ManageInfo } from "../types";

const config = useConfig();
const { setThemeMode } = useTheme(config);
const presets = themePresets;
const modeOptions = themeModeOptions;
const feedback = ref<{ ok: boolean; text: string } | null>(null);

const manageInfo = ref<ManageInfo | null>(null);
const manageLoading = ref(false);

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

async function loadManageInfo() {
  manageLoading.value = true;
  try {
    manageInfo.value = await getManageInfo();
  } catch (e) {
    feedback.value = { ok: false, text: `获取管理目录信息失败: ${e}` };
  } finally {
    manageLoading.value = false;
  }
}

onMounted(() => {
  loadManageInfo();
});
</script>

<template>
  <div class="settings">
    <header class="settings-head">
      <span
        class="head-icon"
        :style="{ background: 'linear-gradient(135deg,#94a3b8,#475569)' }"
        >⚙</span
      >
      <div class="head-text">
        <h2>设置</h2>
        <p>主题 · 管理目录</p>
      </div>
    </header>

    <div v-if="feedback" :class="['banner', feedback.ok ? 'success' : 'error']">
      {{ feedback.text }}
    </div>

    <!-- 主题 -->
    <section class="card">
      <h3>主题</h3>

      <div class="row">
        <span class="row-label">外观模式</span>
        <div class="options">
          <button
            v-for="option in modeOptions"
            :key="option.value"
            class="option-btn"
            :class="{ active: config.themeMode === option.value }"
            @click="setThemeMode(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </div>

      <div class="row">
        <span class="row-label">主题色</span>
        <div class="swatches">
          <button
            v-for="preset in presets"
            :key="preset.name"
            class="swatch"
            :class="{ active: config.themeAccent.toLowerCase() === preset.color.toLowerCase() }"
            :style="{ '--swatch-color': preset.color }"
            :title="preset.name"
            @click="config.themeAccent = preset.color"
          >
            <span class="swatch-dot"></span>
            <span>{{ preset.name }}</span>
          </button>
        </div>
      </div>
    </section>


    <!-- 管理目录 -->
    <section class="card">
      <h3>管理目录</h3>

      <div class="row">
        <span class="row-label">路径</span>
        <code class="row-value path">{{ manageInfo?.path ?? (manageLoading ? "加载中…" : "—") }}</code>
      </div>

      <div class="row">
        <span class="row-label">已管理版本</span>
        <span class="row-value">{{ manageInfo?.versionCount ?? 0 }} 个</span>
      </div>

      <div class="row">
        <span class="row-label">占用空间</span>
        <span class="row-value">{{ manageInfo ? formatBytes(manageInfo.sizeBytes) : "—" }}</span>
      </div>

      <div v-if="manageInfo?.runtimes" class="runtime-detail">
        <div
          v-for="rt in manageInfo.runtimes"
          :key="rt.kind"
          class="runtime-line"
        >
          <span class="rt-icon">{{ RUNTIME_META[rt.kind].icon }}</span>
          <span class="rt-name">{{ RUNTIME_META[rt.kind].name }}</span>
          <span class="rt-versions">
            {{ rt.versions.length ? rt.versions.join("、") : "未安装" }}
          </span>
        </div>
      </div>

      <p class="hint">
        NovaEnv 安装的版本存放在此目录（仅应用管理，不影响系统环境）；卸载、升级均在本目录内操作。
      </p>
    </section>

    <!-- 关于已移至系统菜单（NovaEnv → 关于 NovaEnv） -->
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 680px;
}

.settings-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding-bottom: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.head-icon {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: 12px;
  color: #fff;
  font-size: 20px;
  font-weight: 700;
  box-shadow: var(--shadow-md);
  flex-shrink: 0;
}

.head-text {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

.head-text h2 {
  font-size: var(--text-xl);
  font-weight: 700;
  letter-spacing: -0.01em;
}

.head-text p {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.icon {
  font-size: 22px;
}

h2 {
  font-size: 18px;
}

h3 {
  font-size: 15px;
  margin-bottom: 14px;
}

.card {
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.row-label {
  width: 84px;
  flex-shrink: 0;
  color: var(--text-secondary);
  font-size: 13px;
}

.row-value {
  font-size: 13px;
}

.row-value.path {
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
  color: var(--text-secondary);
  word-break: break-all;
}

.row-hint {
  font-size: 12px;
  color: var(--text-muted);
}

.error-text {
  color: var(--danger);
}

.options {
  display: flex;
  gap: 8px;
}

.option-btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-input);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 6px 14px;
  font-size: 13px;
  transition: all 0.15s;
}

.option-btn.active {
  background: var(--brand-soft);
  border-color: var(--brand);
  color: var(--brand);
  font-weight: 600;
}

.swatches {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.swatch {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-input);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 12px;
  transition: all 0.15s;
}

.swatch.active {
  border-color: var(--swatch-color);
  outline: 2px solid var(--swatch-color);
  outline-offset: 1px;
}

.swatch-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--swatch-color);
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-input);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 7px 16px;
  font-size: 13px;
  transition: background 0.15s;
}

.btn:hover:not(:disabled) {
  background: var(--bg-panel-hover);
}

.btn.primary {
  background: var(--brand);
  border-color: var(--brand);
  color: #04120d;
  font-weight: 600;
}

.btn.primary:hover:not(:disabled) {
  background: var(--brand-hover);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.banner {
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 13px;
}

.banner.error {
  background: var(--danger-soft);
  color: var(--danger);
  border: 1px solid var(--danger);
}

.banner.success {
  background: var(--success-soft);
  color: var(--success);
  border: 1px solid var(--success);
}

.switch {
  position: relative;
  display: inline-block;
  width: 38px;
  height: 22px;
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.switch-track {
  position: absolute;
  inset: 0;
  background: var(--bg-input);
  border: 1px solid var(--border-strong);
  border-radius: 999px;
  transition: background 0.2s;
}

.switch-track::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--text-muted);
  top: 2px;
  left: 2px;
  transition: transform 0.2s, background 0.2s;
}

.switch input:checked + .switch-track {
  background: var(--brand);
  border-color: var(--brand);
}

.switch input:checked + .switch-track::after {
  transform: translateX(16px);
  background: #fff;
}




.progress {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bar {
  height: 8px;
  border-radius: 999px;
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--brand);
  transition: width 0.2s;
}

.msg {
  font-size: 12px;
  color: var(--text-secondary);
}


.runtime-detail {
  border-top: 1px dashed var(--border-subtle);
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.runtime-line {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}

.rt-name {
  width: 70px;
  color: var(--text-secondary);
}

.rt-versions {
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}

.hint {
  font-size: 12px;
  color: var(--text-muted);
  border-top: 1px dashed var(--border-subtle);
  padding-top: 12px;
}

/* 关于区块 */
.link-btn {
  background: none;
  border: none;
  padding: 0;
  color: var(--accent, #34d399);
  font-size: 12px;
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.link-btn:hover {
  opacity: 0.8;
}
.about-copy {
  margin: 12px 0 0;
  font-size: 11px;
  color: var(--text-tertiary, #6b7686);
}
</style>
