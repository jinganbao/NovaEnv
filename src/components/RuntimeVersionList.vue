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

/** 大版本标识：Go 取前两段（1.24），其余取第一段 */
function majorOf(version: string): string {
  if (props.kind === "go") return version.split(".").slice(0, 2).join(".");
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
    await installVersion(props.kind, target);
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
      <h3>可用版本（按大版本）</h3>
      <button class="btn mini" :disabled="loading || busy" @click="load(true)">
        {{ loading ? "获取中…" : "刷新列表" }}
      </button>
    </div>

    <div v-if="error" class="hint error">{{ error }}</div>
    <div v-else-if="loading" class="hint">正在从官方源获取可用版本…</div>

    <div v-else class="groups">
      <div
        v-for="g in merged"
        :key="g.major"
        class="group"
        :class="{ 'has-installed': installedOf(g.major).length > 0 }"
      >
        <div class="group-row">
          <span class="g-name">{{ RUNTIME_META[kind].name }} {{ g.major }}</span>
          <span v-if="g.isLts" class="badge lts">LTS</span>
          <span class="g-latest">最新 {{ g.latest || "—" }}</span>
          <div class="g-actions">
            <span
              v-if="installedOf(g.major).length && !hasNewer(g)"
              class="ok-text"
            >
              已是最新 ✓
            </span>
            <button
              v-if="!installedOf(g.major).length"
              class="btn primary"
              :disabled="busy"
              @click="doInstall(g, g.latest)"
            >
              {{ busy && installingMajor === g.major ? "安装中…" : "安装" }}
            </button>
            <button
              v-if="installedOf(g.major).length && hasNewer(g)"
              class="btn primary"
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

        <!-- 该大版本下已安装的小版本 -->
        <div v-if="installedOf(g.major).length" class="g-installed">
          <span class="label">已安装：</span>
          <span
            v-for="v in installedOf(g.major)"
            :key="v.path"
            class="installed-chip"
          >
            <span class="chip-ver" :class="{ def: v.isDefault }">
              {{ v.version }}{{ v.isDefault ? "（默认）" : "" }}
            </span>
            <button
              v-if="!v.isDefault"
              class="mini-btn"
              @click="emit('activate', v)"
            >
              设为默认
            </button>
            <button
              v-if="v.managed && !v.isDefault"
              class="mini-btn danger"
              @click="emit('uninstall', v)"
            >
              卸载
            </button>
          </span>
        </div>

        <!-- 该组正在安装/升级的进度 -->
        <div
          v-if="busy && installingMajor === g.major && progress"
          class="progress"
        >
          <div class="bar">
            <div
              class="fill"
              :style="{ width: (progress.percent ?? 0) + '%' }"
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
  gap: 12px;
}

.panel-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

h3 {
  font-size: 15px;
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  transition: background 0.15s;
}

.btn.mini {
  padding: 4px 10px;
  font-size: 12px;
}

.btn.primary {
  background: var(--brand);
  border-color: var(--brand);
  color: #fff;
}

.btn.primary:hover:not(:disabled) {
  background: var(--brand-hover);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.hint {
  color: var(--text-secondary);
  font-size: 13px;
}

.hint.error {
  color: var(--danger);
}

.groups {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.group {
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-app);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.group.has-installed {
  border-color: var(--success);
}

.group-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.g-name {
  font-size: 15px;
  font-weight: 700;
}

.badge.lts {
  background: var(--success);
  color: #fff;
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 999px;
}

.g-latest {
  color: var(--text-secondary);
  font-size: 12px;
}

.g-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
}

.ok-text {
  color: var(--success);
  font-size: 13px;
  font-weight: 600;
}

.g-installed {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.label {
  color: var(--text-secondary);
  font-size: 12px;
}

.installed-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  border-radius: 8px;
  padding: 4px 8px;
}

.chip-ver {
  font-size: 12px;
  font-weight: 600;
}

.chip-ver.def {
  color: var(--success);
}

.mini-btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  color: var(--text-primary);
  border-radius: 6px;
  padding: 2px 8px;
  font-size: 11px;
  cursor: pointer;
}

.mini-btn:hover {
  background: var(--bg-panel-hover);
}

.mini-btn.danger {
  color: var(--danger);
  border-color: var(--danger);
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

.result {
  font-size: 13px;
}

.result.ok {
  color: var(--success);
}

.result.fail {
  color: var(--danger);
}
</style>
