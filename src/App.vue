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

const kinds: RuntimeKind[] = ["java", "node", "go", "maven"];
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
useAppUpdate(config, {
  error: (m) => showToast(false, m),
  success: () => {},
}).autoCheckOnStartup();

const counts = computed<Record<RuntimeKind, number>>(() => {
  const c: Record<RuntimeKind, number> = { java: 0, node: 0, go: 0, maven: 0 };
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
        <span class="logo">N</span>
        <div>
          <h1>NovaEnv</h1>
          <p class="subtitle">开发环境可视化管理</p>
        </div>
      </div>
      <div class="overview-chips">
        <div v-for="k in kinds" :key="k" class="chip" :title="`${RUNTIME_META[k].name} 当前版本`">
          <span class="chip-icon">{{ RUNTIME_META[k].icon }}</span>
          <span class="chip-ver">{{ overviewValue(k) }}</span>
        </div>
      </div>
      <button class="btn" :disabled="loading" @click="refresh">
        {{ loading ? "扫描中…" : "重新扫描" }}
      </button>
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
  gap: 16px;
  padding: 12px 24px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-panel);
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
}

.logo {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: var(--brand);
  color: #fff;
  font-size: 22px;
  font-weight: 700;
}

h1 {
  font-size: 19px;
  line-height: 1.2;
}

.subtitle {
  color: var(--text-secondary);
  font-size: 12px;
}

.overview-chips {
  display: flex;
  gap: 8px;
  margin-left: auto;
}

.chip {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  border-radius: 999px;
  padding: 4px 12px;
  font-size: 12px;
}

.chip-ver {
  font-weight: 600;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 8px 16px;
  font-size: 13px;
  transition: background 0.15s;
}

.btn:hover:not(:disabled) {
  background: var(--bg-panel-hover);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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
  padding: 24px 28px 40px;
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
