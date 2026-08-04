<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  activate,
  availableVersions,
  listRuntimes,
  listServices,
  previewActivation,
  uninstallVersion,
} from "./api";
import type {
  ActivationPreview,
  RuntimesPayload,
  RuntimeKind,
  RuntimeVersion,
  ServiceInfo,
} from "./types";
import { RUNTIME_META } from "./types";
import { useAppUpdate } from "./composables/useAppUpdate";
import { useConfig } from "./composables/useConfig";
import { useTheme } from "./composables/useTheme";
import ActivationModal from "./components/ActivationModal.vue";
import RuntimeDetail from "./components/RuntimeDetail.vue";
import ServiceDetail from "./components/ServiceDetail.vue";
import SettingsView from "./components/SettingsView.vue";
import Sidebar from "./components/Sidebar.vue";
import UninstallModal from "./components/UninstallModal.vue";

const kinds: RuntimeKind[] = ["java", "node", "go", "maven", "python"];
const serviceKinds = ["redis", "mysql"] as const;

type Selected = RuntimeKind | (typeof serviceKinds)[number] | "settings";

// 配置与主题（启动即应用持久化的主题设置）
const config = useConfig();
useTheme(config);

const loading = ref(false);
const error = ref<string | null>(null);
const payload = ref<RuntimesPayload | null>(null);
const services = ref<ServiceInfo[]>([]);
const selected = ref<Selected>("java");

// 切换默认流程
const pendingVersion = ref<RuntimeVersion | null>(null);
const pendingPreview = ref<ActivationPreview | null>(null);
const activating = ref(false);

// 卸载流程
const uninstallTarget = ref<RuntimeVersion | null>(null);
const uninstalling = ref(false);

const toast = ref<{ ok: boolean; text: string } | null>(null);
let toastTimer: number | undefined;

/** 显示短暂提示（toast），3 秒后自动消失 */
function showToast(ok: boolean, text: string) {
  toast.value = { ok, text };
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (toast.value = null), 3000);
}

function clearToast() {
  window.clearTimeout(toastTimer);
  toast.value = null;
}

// 启动时静默检查更新
const update = useAppUpdate(config, {
  error: (m) => showToast(false, m),
  success: () => {},
});
update.autoCheckOnStartup();
update.loadCurrentVersion();
/** 当前应用版本（供 Header / 侧边栏展示） */
const appVersion = computed(() => update.currentVersion.value || "1.0.5");

const counts = computed<Record<RuntimeKind, number>>(() => {
  const c: Record<RuntimeKind, number> = {
    java: 0,
    node: 0,
    go: 0,
    maven: 0,
    python: 0,
  };
  for (const v of payload.value?.versions ?? []) c[v.kind]++;
  return c;
});

function versionsOf(kind: RuntimeKind): RuntimeVersion[] {
  return payload.value?.versions.filter((v) => v.kind === kind) ?? [];
}

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    payload.value = await listRuntimes();
  } catch (e) {
    error.value = `扫描失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

// ---- 服务类组件 ----

async function loadServices() {
  try {
    services.value = await listServices();
  } catch {
    // 服务状态轮询失败静默（不打断主界面）
  }
}

function serviceOf(kind: ServiceInfo["kind"]): ServiceInfo | undefined {
  return services.value.find((s) => s.kind === kind);
}

// 服务列表为空时的兜底展示（正常情况 list_services 恒返回各项）
function emptyService(kind: ServiceInfo["kind"]): ServiceInfo {
  return {
    kind,
    name: kind === "mysql" ? "MySQL" : "Redis",
    installed: false,
    version: null,
    versions: [],
    running: false,
    port: kind === "mysql" ? 3306 : 6379,
    pid: null,
    password: "",
    autostart: false,
    dataDir: "",
    note: null,
  };
}

// 服务状态轮询（3s），保证状态点与详情页实时性
let pollTimer: number | undefined;
function startPolling() {
  loadServices();
  pollTimer = window.setInterval(loadServices, 3000);
}

// ---- 切换默认 ----

async function openActivation(version: RuntimeVersion) {
  clearToast();
  try {
    pendingPreview.value = await previewActivation(version);
    pendingVersion.value = version;
  } catch (e) {
    showToast(false, `生成预览失败: ${e}`);
  }
}

async function doActivate() {
  if (!pendingVersion.value) return;
  activating.value = true;
  clearToast();
  try {
    await activate(pendingVersion.value);
    showToast(true, `已将 ${pendingVersion.value.version} 设为默认，新打开的终端生效。`);
    pendingVersion.value = null;
    pendingPreview.value = null;
    await refresh();
  } catch (e) {
    showToast(false, `切换失败: ${e}`);
  } finally {
    activating.value = false;
  }
}

function closeActivation() {
  pendingVersion.value = null;
  pendingPreview.value = null;
}

// ---- 卸载 ----

function openUninstall(version: RuntimeVersion) {
  clearToast();
  uninstallTarget.value = version;
}

async function doUninstall() {
  if (!uninstallTarget.value) return;
  uninstalling.value = true;
  clearToast();
  try {
    await uninstallVersion(uninstallTarget.value);
    showToast(true, "卸载成功");
    uninstallTarget.value = null;
    await refresh();
  } catch (e) {
    showToast(false, `卸载失败: ${e}`);
  } finally {
    uninstalling.value = false;
  }
}

function closeUninstall() {
  uninstallTarget.value = null;
}

function overviewValue(kind: RuntimeKind): string {
  const value = payload.value?.overview[kind];
  return value && value.trim() !== "" ? value : "未检测到";
}

onMounted(() => {
  refresh();
  startPolling();
  // 后台并行预加载各环境的可用版本列表（写入后端缓存），
  // 首次切换环境时无需再等待网络请求
  for (const k of kinds) {
    availableVersions(k).catch(() => {});
  }
});

onUnmounted(() => {
  window.clearInterval(pollTimer);
});
</script>

<template>
  <div class="layout">
    <header class="header">
      <div class="brand">
        <svg class="logo" width="36" height="36" viewBox="0 0 36 36" aria-hidden="true">
          <defs>
            <linearGradient id="brand-g" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#34d399" />
              <stop offset="1" stop-color="#0d9488" />
            </linearGradient>
          </defs>
          <rect width="36" height="36" rx="10" fill="url(#brand-g)" />
          <path
            d="M12 10v16M12 10l11 8-11 8"
            fill="none"
            stroke="#06281f"
            stroke-width="3.2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <div class="brand-text">
          <h1>NovaEnv</h1>
          <p class="subtitle">开发环境可视化管理</p>
        </div>
        <span class="badge badge-brand">v{{ appVersion }}</span>
      </div>

      <div class="header-right">
        <div class="overview-chips">
          <div v-for="k in kinds" :key="k" class="chip" :title="`${RUNTIME_META[k].name} 当前版本`">
            <span
              class="chip-icon"
              :style="{ background: RUNTIME_META[k].color }"
              >{{ RUNTIME_META[k].letter }}</span
            >
            <span class="chip-ver">{{ overviewValue(k) }}</span>
          </div>
        </div>
        <button class="btn btn-ghost" :disabled="loading" @click="refresh">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <path d="M21 3v6h-6" />
          </svg>
          {{ loading ? "扫描中…" : "重新扫描" }}
        </button>
      </div>
    </header>

    <div v-if="error" class="banner error">{{ error }}</div>

    <Transition name="toast">
      <div v-if="toast" :class="['toast', toast.ok ? 'toast-ok' : 'toast-err']">
        <span class="toast-icon">{{ toast.ok ? "✓" : "✕" }}</span>
        <span class="toast-text">{{ toast.text }}</span>
      </div>
    </Transition>

    <div class="body">
      <Sidebar
        :kinds="kinds"
        :selected="selected"
        :counts="counts"
        :services="services"
        @select="selected = $event"
      />

      <main class="content">
        <div
          v-if="!payload && loading && selected !== 'settings' && selected !== 'redis'"
          class="placeholder"
        >
          正在扫描本机开发环境…
        </div>
        <SettingsView v-else-if="selected === 'settings'" />
        <ServiceDetail
          v-else-if="selected === 'redis' || selected === 'mysql'"
          :key="selected"
          :service="serviceOf(selected) ?? emptyService(selected)"
          @refresh="loadServices"
        />
        <RuntimeDetail
          v-else-if="payload"
          :kind="selected"
          :versions="versionsOf(selected)"
          @activate="openActivation"
          @uninstall="openUninstall"
          @refresh="refresh"
        />
      </main>
    </div>

    <ActivationModal
      v-if="pendingVersion && pendingPreview"
      :version="pendingVersion"
      :preview="pendingPreview"
      :busy="activating"
      @confirm="doActivate"
      @cancel="closeActivation"
    />

    <UninstallModal
      v-if="uninstallTarget"
      :version="uninstallTarget"
      :busy="uninstalling"
      @confirm="doUninstall"
      @cancel="closeUninstall"
    />
  </div>
</template>

<style scoped>
.layout {
  height: 100%;
  min-height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.header {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  flex-shrink: 0;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.25;
}

h1 {
  font-size: var(--text-lg);
  font-weight: 700;
  letter-spacing: -0.01em;
}

.subtitle {
  color: var(--text-secondary);
  font-size: var(--text-xs);
}

.header-right {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin-left: auto;
}

.overview-chips {
  display: flex;
  gap: var(--space-2);
}

.chip {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  border-radius: var(--radius-pill);
  padding: 3px 10px 3px 4px;
  font-size: var(--text-xs);
}

.chip-icon {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 5px;
  color: #fff;
  font-size: 10px;
  font-weight: 700;
}

.chip-ver {
  font-weight: 600;
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.banner {
  border-radius: 8px;
  padding: 10px 14px;
  margin: 14px 24px 0;
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

.body {
  flex: 1;
  display: flex;
  min-height: 0;
}

.content {
  flex: 1;
  min-width: 0;
  padding: var(--space-6) var(--space-6) var(--space-8);
  overflow-y: auto;
}

.placeholder {
  text-align: center;
  color: var(--text-secondary);
  padding: 80px 0;
}

/* ---- Toast（短暂提示，3 秒自动消失）---- */
.toast {
  position: fixed;
  top: 18px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 300;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: min(480px, 90vw);
  padding: 10px 18px;
  border-radius: 10px;
  font-size: 13px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
}

.toast-ok {
  border-color: var(--success);
}

.toast-err {
  border-color: var(--danger);
}

.toast-icon {
  font-size: 14px;
  font-weight: 700;
  flex-shrink: 0;
}

.toast-ok .toast-icon {
  color: var(--success);
}

.toast-err .toast-icon {
  color: var(--danger);
}

.toast-text {
  color: var(--text-primary);
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
}
</style>
