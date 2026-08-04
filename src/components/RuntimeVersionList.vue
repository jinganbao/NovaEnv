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
const result = ref<{ ok: boolean; text: string } | null>(null);
const installingMajor = ref("");

let unlisten: (() => void) | null = null;

/** 大版本标识：Go/Python 取前两段（1.24 / 3.13），其余取第一段 */
function majorOf(version: string): string {
  if (props.kind === "go" || props.kind === "python") {
    return version.split(".").slice(0, 2).join(".");
  }
  return version.split(".")[0];
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

/** 已安装版本按大版本索引 */
const installedByMajor = computed(() => {
  const map = new Map<string, RuntimeVersion[]>();
  for (const v of props.installed) {
    const major = majorOf(v.version);
    if (!map.has(major)) map.set(major, []);
    map.get(major)!.push(v);
  }
  return map;
});

/** 官方分组 + 已安装版本（可能不在官方列表前 N 条内）合并 */
const merged = computed<AvailableVersionGroup[]>(() => {
  const list = groups.value.map((g) => ({ ...g, versions: [...g.versions] }));
  for (const [major, installed] of installedByMajor.value) {
    let g = list.find((x) => x.major === major);
    if (!g) {
      g = { major, isLts: false, versions: [], latest: "" };
      list.push(g);
    }
    for (const v of installed) {
      if (!g.versions.includes(v.version)) g.versions.push(v.version);
    }
    g.versions.sort((a, b) => compareVersions(b, a));
    if (!g.latest) g.latest = g.versions[0];
  }
  return list.sort((a, b) => compareVersions(b.major, a.major));
});

function installedOf(major: string): RuntimeVersion[] {
  return installedByMajor.value.get(major) ?? [];
}

/** 分组展开状态（小版本多时折叠，默认显示前 6 条） */
const expanded = ref<Record<string, boolean>>({});
const PREVIEW_LIMIT = 6;

function visibleVersions(g: AvailableVersionGroup): string[] {
  if (expanded.value[g.major] || g.versions.length <= PREVIEW_LIMIT) {
    return g.versions;
  }
  return g.versions.slice(0, PREVIEW_LIMIT);
}

function toggleExpand(g: AvailableVersionGroup) {
  expanded.value = { ...expanded.value, [g.major]: !expanded.value[g.major] };
}

/** 某版本是否已安装 */
function isInstalled(version: string): RuntimeVersion | undefined {
  return props.installed.find((v) => v.version === version);
}

/** 该大版本已装最新版本是否落后于官方最新（可升级） */
function hasNewer(g: AvailableVersionGroup): boolean {
  if (!g.latest) return false;
  const installed = installedOf(g.major);
  if (!installed.length) return false;
  const newest = [...installed].sort((a, b) =>
    compareVersions(b.version, a.version),
  )[0];
  return compareVersions(newest.version, g.latest) < 0;
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

async function doInstall(g: AvailableVersionGroup, target: string) {
  if (busy.value || !target) return;
  busy.value = true;
  installingMajor.value = g.major;
  result.value = null;
  progress.value = null;
  try {
    const res = await installVersion(props.kind, target);
    let text = `安装完成：${target}`;
    if (res.removed.length) {
      text = `已安装 ${target}，自动替换同大版本旧版本：${res.removed.join("、")}`;
    }
    if (res.promoted) text += "，并已自动设为默认";
    result.value = { ok: true, text };
  } catch (e) {
    result.value = { ok: false, text: `安装失败: ${e}` };
  } finally {
    busy.value = false;
    installingMajor.value = "";
    emit("refresh");
  }
}

// 切换左侧环境时组件实例复用，需监听 kind 重新拉取对应环境的版本列表
watch(
  () => props.kind,
  () => {
    busy.value = false;
    installingMajor.value = "";
    progress.value = null;
    result.value = null;
    load();
  },
);

onMounted(() => {
  load();
  onInstallProgress((p) => {
    if (p.kind !== props.kind) return;
    progress.value = p;
    if (p.stage === "done") {
      result.value = { ok: true, text: `安装完成：${RUNTIME_META[props.kind].name} ${p.version}` };
    } else if (p.stage === "error") {
      result.value = { ok: false, text: p.message };
    }
  }).then((fn) => (unlisten = fn));
});

onUnmounted(() => unlisten?.());
</script>

<template>
  <section class="version-list">
    <div class="panel-head">
      <div class="panel-head-text">
        <h3>可用版本</h3>
        <span class="panel-sub">按大版本分组 · 同大版本自动保留最新</span>
      </div>
      <button class="btn btn-ghost btn-sm" :disabled="loading || busy" @click="load(true)">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M21 12a9 9 0 1 1-2.64-6.36" />
          <path d="M21 3v6h-6" />
        </svg>
        {{ loading ? "获取中…" : "刷新列表" }}
      </button>
    </div>

    <div v-if="error" class="hint error">{{ error }}</div>
    <div v-else-if="loading" class="hint">正在从官方源获取可用版本…</div>

    <div v-else class="groups">
      <div
        v-for="g in merged"
        :key="g.major"
        class="group card"
        :class="{ 'has-installed': installedOf(g.major).length > 0 }"
      >
        <div class="group-head">
          <span
            class="g-badge"
            :style="{ background: RUNTIME_META[kind].color }"
            >{{ RUNTIME_META[kind].letter }}</span
          >
          <span class="g-name">{{ RUNTIME_META[kind].name }} {{ g.major }}</span>
          <span v-if="g.isLts" class="badge badge-brand">LTS</span>
          <span class="g-latest">最新 {{ g.latest || "—" }}</span>

          <div class="g-actions">
            <span v-if="installedOf(g.major).length && !hasNewer(g)" class="ok-text">
              已是最新
            </span>
            <button
              v-if="!installedOf(g.major).length"
              class="btn btn-primary btn-sm"
              :disabled="busy"
              @click="doInstall(g, g.latest)"
            >
              {{ busy && installingMajor === g.major ? "安装中…" : "安装" }}
            </button>
            <button
              v-if="installedOf(g.major).length && hasNewer(g)"
              class="btn btn-primary btn-sm"
              :disabled="busy"
              @click="doInstall(g, g.latest)"
            >
              {{
                busy && installingMajor === g.major
                  ? "升级中…"
                  : `升级到 ${g.latest}`
              }}
            </button>
          </div>
        </div>

        <!-- 该大版本下的全部版本行 -->
        <div class="g-versions">
          <div
            v-for="ver in visibleVersions(g)"
            :key="ver"
            class="ver-row"
            :class="{ installed: !!isInstalled(ver) }"
          >
            <span class="ver-name">{{ ver }}</span>
            <span v-if="ver === g.latest && !isInstalled(ver)" class="badge badge-warning">
              最新
            </span>
            <span v-if="isInstalled(ver)" class="badge badge-success">
              {{ isInstalled(ver)!.isDefault ? "默认" : "已安装" }}
            </span>
            <div class="ver-actions">
              <button
                v-if="isInstalled(ver) && !isInstalled(ver)!.isDefault"
                class="btn btn-sm"
                :disabled="busy"
                @click="emit('activate', isInstalled(ver)!)"
              >
                设为默认
              </button>
              <button
                v-if="isInstalled(ver) && isInstalled(ver)!.managed && !isInstalled(ver)!.isDefault"
                class="btn btn-sm btn-danger"
                :disabled="busy"
                @click="emit('uninstall', isInstalled(ver)!)"
              >
                卸载
              </button>
            </div>
          </div>
          <button
            v-if="g.versions.length > PREVIEW_LIMIT"
            class="expand-btn"
            @click="toggleExpand(g)"
          >
            {{ expanded[g.major] ? "收起" : `显示全部 ${g.versions.length} 个版本` }}
          </button>
        </div>

        <!-- 该组正在安装/升级的进度 -->
        <div v-if="busy && installingMajor === g.major && progress" class="progress">
          <div class="bar">
            <div
              class="fill"
              :class="{ indeterminate: progress.percent == null }"
              :style="progress.percent != null ? { width: progress.percent + '%' } : {}"
            ></div>
          </div>
          <span class="msg">{{ progress.message }}</span>
        </div>
      </div>

      <div v-if="result" :class="['result', result.ok ? 'ok' : 'fail']">
        {{ result.text }}
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

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.panel-head-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

h3 {
  font-size: var(--text-lg);
  font-weight: 700;
}

.panel-sub {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.hint {
  color: var(--text-secondary);
  font-size: var(--text-md);
  padding: var(--space-3) 0;
}

.hint.error {
  color: var(--danger);
}

.groups {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

/* ---- 分组卡 ---- */
.group {
  overflow: hidden;
  transition: border-color var(--duration) var(--ease);
}

.group.has-installed {
  border-color: rgba(52, 211, 153, 0.35);
}

.group-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.g-badge {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 8px;
  color: #fff;
  font-size: 13px;
  font-weight: 700;
  flex-shrink: 0;
}

.g-name {
  font-size: var(--text-lg);
  font-weight: 700;
}

.g-latest {
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.g-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.ok-text {
  font-size: var(--text-sm);
  color: var(--success);
  font-weight: 600;
}

/* ---- 版本行 ---- */
.g-versions {
  display: flex;
  flex-direction: column;
}

.ver-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 16px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background var(--duration) var(--ease);
}

.ver-row:last-child {
  border-bottom: none;
}

.ver-row:hover {
  background: var(--bg-panel-hover);
}

.ver-row.installed {
  background: rgba(52, 211, 153, 0.06);
}

.ver-name {
  font-family: "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace;
  font-size: var(--text-md);
  font-weight: 600;
}

.ver-actions {
  margin-left: auto;
  display: flex;
  gap: 6px;
}

/* ---- 进度条 ---- */
.progress {
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bar {
  height: 6px;
  border-radius: 999px;
  background: var(--bg-input);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--brand);
  border-radius: 999px;
  transition: width 0.3s var(--ease);
}

.fill.indeterminate {
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

.msg {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.result {
  font-size: var(--text-md);
  border-radius: var(--radius-md);
  padding: 10px 14px;
  border: 1px solid;
}

.result.ok {
  color: var(--success);
  border-color: transparent;
  background: var(--success-soft);
}

.result.fail {
  color: var(--danger);
  border-color: transparent;
  background: var(--danger-soft);
}

.expand-btn {
  border: none;
  background: transparent;
  color: var(--brand);
  font-size: var(--text-sm);
  padding: 8px;
  width: 100%;
  border-top: 1px solid var(--border-subtle);
  transition: background var(--duration) var(--ease);
}

.expand-btn:hover {
  background: var(--brand-soft);
}
</style>
