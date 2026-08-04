<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { availableVersions, installVersion, onInstallProgress } from "../api";
import type {
  AvailableVersionGroup,
  InstallProgress,
  RuntimeKind,
  RuntimeVersion,
} from "../types";
import { RUNTIME_META } from "../types";

const props = defineProps<{
  kind: RuntimeKind;
  installed: RuntimeVersion[];
}>();
const emit = defineEmits<{
  (e: "activate", version: RuntimeVersion): void;
  (e: "uninstall", version: RuntimeVersion): void;
  (e: "refresh"): void;
}>();

const groups = ref<AvailableVersionGroup[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const busy = ref(false);
const progress = ref<InstallProgress | null>(null);
const installTarget = ref("");
/** 操作反馈 toast */
const toast = ref<{ ok: boolean; text: string } | null>(null);
let toastTimer: number | undefined;

let unlisten: (() => void) | null = null;

function showToast(ok: boolean, text: string) {
  toast.value = { ok, text };
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (toast.value = null), ok ? 2500 : 5000);
}

/** 数字段逐位比较版本号 */
function compareVersions(a: string, b: string): number {
  const nums = (s: string) => s.match(/\d+/g)?.map(Number) ?? [];
  const na = nums(a);
  const nb = nums(b);
  const len = Math.max(na.length, nb.length);
  for (let i = 0; i < len; i++) {
    const x = na[i] ?? 0;
    const y = nb[i] ?? 0;
    if (x !== y) return x - y;
  }
  return 0;
}

/** 全部可用版本平铺（倒序，最新在前；已安装标注禁用） */
const flatVersions = computed(() => {
  const flat: { version: string; installed: boolean; isLts: boolean }[] = [];
  for (const g of groups.value) {
    for (const v of g.versions) {
      flat.push({
        version: v,
        installed: !!props.installed.find((s) => s.version === v),
        isLts: g.isLts,
      });
    }
  }
  return flat.sort((a, b) => compareVersions(b.version, a.version));
});

/** 来源展示名 */
function vendorName(vendor: string): string {
  switch (vendor) {
    case "novaenv":
      return "NovaEnv";
    case "homebrew":
      return "Homebrew";
    case "nvm":
    case "nvm-windows":
      return "nvm";
    case "fnm":
      return "fnm";
    case "system":
      return "系统";
    default:
      return vendor;
  }
}

async function load(refresh = false) {
  loading.value = true;
  error.value = null;
  try {
    groups.value = await availableVersions(props.kind, refresh);
  } catch (e) {
    error.value = `获取可用版本失败: ${e}`;
  } finally {
    loading.value = false;
  }
}

async function doInstall() {
  const target = installTarget.value;
  if (busy.value || !target) return;
  busy.value = true;
  progress.value = null;
  try {
    const res = await installVersion(props.kind, target);
    let text = `安装完成：${target}`;
    if (res.removed.length) {
      text = `已安装 ${target}，自动替换同大版本旧版本：${res.removed.join("、")}`;
    }
    if (res.promoted) text += "，并已自动设为默认";
    showToast(true, text);
    emit("refresh");
  } catch (e) {
    showToast(false, `安装失败: ${e}`);
  } finally {
    busy.value = false;
  }
}

// 切换左侧环境时组件实例复用，需监听 kind 重新拉取对应环境的版本列表
watch(
  () => props.kind,
  () => {
    busy.value = false;
    progress.value = null;
    installTarget.value = "";
    load();
  },
);

onMounted(() => {
  load();
  onInstallProgress((p) => {
    if (p.kind !== props.kind) return;
    progress.value = p;
    if (p.stage === "done") {
      showToast(true, `安装完成：${RUNTIME_META[props.kind].name} ${p.version}`);
    } else if (p.stage === "error") {
      showToast(false, p.message);
    }
  }).then((fn) => (unlisten = fn));
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <section class="version-list">
    <!-- 操作反馈 Toast -->
    <Transition name="toast">
      <div
        v-if="toast"
        class="op-toast"
        :class="toast.ok ? 'op-toast-ok' : 'op-toast-err'"
      >
        <span class="op-toast-icon">{{ toast.ok ? "✓" : "✕" }}</span>
        <span>{{ toast.text }}</span>
      </div>
    </Transition>

    <!-- 安装面板：版本下拉 -->
    <div class="card install-panel">
      <div class="install-head">
        <p class="install-tip">
          安装新版本（同大版本自动保留最新）
        </p>
      </div>
      <div class="install-row">
        <label class="cfg-field cfg-version">
          <span class="cfg-label">版本</span>
          <select
            v-model="installTarget"
            class="cfg-input cfg-select"
            :disabled="busy || loading"
          >
            <option value="" disabled>选择版本…</option>
            <option
              v-for="ver in flatVersions"
              :key="ver.version"
              :value="ver.version"
              :disabled="ver.installed"
            >
              {{ ver.version }}{{ ver.isLts ? "（LTS）" : "" }}{{ ver.installed ? "（已安装）" : "" }}
            </option>
          </select>
        </label>
        <button class="btn btn-primary" :disabled="busy || !installTarget" @click="doInstall()">
          {{ busy ? "安装中…" : "安装" }}
        </button>
        <button class="btn btn-ghost" :disabled="busy || loading" @click="load(true)">
          {{ loading ? "获取中…" : "刷新列表" }}
        </button>
      </div>
      <div v-if="error" class="install-error">{{ error }}</div>
    </div>

    <!-- 安装进度 -->
    <div v-if="progress && progress.stage !== 'done'" class="card progress-card">
      <div class="progress-head">
        <span class="progress-msg">{{ progress.message }}</span>
        <span v-if="progress.percent != null" class="progress-pct">{{ progress.percent }}%</span>
      </div>
      <div class="bar">
        <div
          class="bar-fill"
          :class="{ indeterminate: progress.percent == null }"
          :style="progress.percent != null ? { width: `${progress.percent}%` } : {}"
        ></div>
      </div>
    </div>

    <!-- 已安装版本（表格形式） -->
    <div v-if="props.installed.length" class="card">
      <div class="installed-head">
        <span class="installed-title">已安装版本</span>
        <span class="muted">{{ props.installed.length }} 个</span>
      </div>
      <div class="ver-table">
        <div class="ver-table-head">
          <span class="col-ver">版本</span>
          <span class="col-src">来源</span>
          <span class="col-status">状态</span>
          <span class="col-ops">操作</span>
        </div>
        <div
          v-for="v in props.installed"
          :key="v.path"
          class="ver-table-row"
          :class="{ 'row-default': v.isDefault }"
        >
          <span class="col-ver ver-name">{{ v.version }}</span>
          <span class="col-src">
            <span v-if="v.managed" class="badge badge-brand badge-tiny">NovaEnv</span>
            <span v-else class="muted">{{ vendorName(v.vendor) }}</span>
          </span>
          <span class="col-status">
            <span
              class="run-dot"
              :class="v.isDefault ? 'run-dot-on' : 'run-dot-off'"
            ></span>
            {{ v.isDefault ? "默认" : "—" }}
          </span>
          <span class="col-ops">
            <button
              v-if="!v.isDefault"
              class="btn btn-sm"
              :disabled="busy"
              @click="emit('activate', v)"
            >
              设为默认
            </button>
            <button
              v-if="v.managed && !v.isDefault"
              class="btn btn-sm btn-danger"
              :disabled="busy"
              @click="emit('uninstall', v)"
            >
              卸载
            </button>
          </span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.version-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.muted {
  font-size: var(--text-sm);
  color: var(--text-muted);
}

/* ---- 安装面板 ---- */
.install-panel {
  padding: var(--space-4) var(--space-5);
  border-color: var(--border-strong);
}

.install-head {
  margin-bottom: var(--space-3);
}

.install-tip {
  font-size: var(--text-md);
  color: var(--text-secondary);
}

.install-row {
  display: flex;
  align-items: flex-end;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.cfg-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.cfg-version {
  min-width: 240px;
  flex: 1;
  max-width: 340px;
}

.cfg-label {
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.cfg-input {
  border: 1px solid var(--border-strong);
  background: var(--bg-input);
  color: var(--text-primary);
  border-radius: var(--radius-md);
  padding: 8px 12px;
  font-size: var(--text-md);
  font-family: inherit;
  outline: none;
  transition: border-color var(--duration) var(--ease);
}

.cfg-input:focus {
  border-color: var(--brand);
}

.cfg-select {
  appearance: none;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%239aa6b7' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'><path d='M6 9l6 6 6-6'/></svg>");
  background-repeat: no-repeat;
  background-position: right 10px center;
  padding-right: 30px;
  cursor: pointer;
}

.install-error {
  margin-top: var(--space-2);
  font-size: var(--text-sm);
  color: var(--danger);
}

/* ---- 进度 ---- */
.progress-card {
  padding: var(--space-4) var(--space-5);
}

.progress-head {
  display: flex;
  justify-content: space-between;
  margin-bottom: var(--space-2);
}

.progress-msg {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.progress-pct {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--brand);
}

.bar {
  height: 6px;
  border-radius: 999px;
  background: var(--bg-input);
  overflow: hidden;
}

.bar-fill {
  height: 100%;
  background: var(--brand);
  border-radius: 999px;
  transition: width 0.3s var(--ease);
}

.bar-fill.indeterminate {
  width: 40% !important;
  animation: slide 1.2s infinite var(--ease);
}

@keyframes slide {
  0% {
    margin-left: -40%;
  }
  100% {
    margin-left: 100%;
  }
}

/* ---- 已安装表格 ---- */
.installed-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px 8px;
}

.installed-title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.ver-table {
  display: flex;
  flex-direction: column;
}

.ver-table-head {
  display: grid;
  grid-template-columns: 140px 120px 100px 1fr;
  gap: var(--space-2);
  align-items: center;
  padding: 8px 16px;
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
  background: var(--bg-app);
  border-top: 1px solid var(--border-subtle);
  border-bottom: 1px solid var(--border-subtle);
}

.ver-table-row {
  display: grid;
  grid-template-columns: 140px 120px 100px 1fr;
  gap: var(--space-2);
  align-items: center;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background var(--duration) var(--ease);
}

.ver-table-row:last-child {
  border-bottom: none;
}

.ver-table-row:hover {
  background: var(--bg-panel-hover);
}

.ver-table-row.row-default {
  background: rgba(52, 211, 153, 0.05);
}

.ver-name {
  font-family: "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace;
  font-size: var(--text-md);
  font-weight: 600;
}

.col-status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-sm);
}

.col-ops {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}

.run-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.run-dot-on {
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
}

.run-dot-off {
  background: var(--text-muted);
}

.badge-tiny {
  padding: 1px 7px;
  font-size: 10px;
}

/* ---- Toast ---- */
.op-toast {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-radius: var(--radius-md);
  font-size: var(--text-md);
  font-weight: 600;
  box-shadow: var(--shadow-lg);
  border: 1px solid;
  max-width: 380px;
}

.op-toast-ok {
  background: var(--bg-panel);
  border-color: var(--success);
  color: var(--success);
}

.op-toast-err {
  background: var(--bg-panel);
  border-color: var(--danger);
  color: var(--danger);
}

.op-toast-icon {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  font-size: 11px;
  flex-shrink: 0;
}

.op-toast-ok .op-toast-icon {
  background: var(--success-soft);
}

.op-toast-err .op-toast-icon {
  background: var(--danger-soft);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s var(--ease), transform 0.2s var(--ease);
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>
