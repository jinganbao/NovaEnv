<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  availableServiceVersions,
  installService,
  onServiceProgress,
  restartService,
  startService,
  stopService,
  uninstallService,
} from "../api";
import type { AvailableVersionGroup, ServiceInfo, ServiceProgress } from "../types";

const props = defineProps<{ service: ServiceInfo }>();
const emit = defineEmits<{ (e: "refresh"): void }>();

const versions = ref<AvailableVersionGroup[]>([]);
const loadingVersions = ref(false);
const busy = ref(false);
const result = ref<{ ok: boolean; text: string } | null>(null);
const progress = ref<ServiceProgress | null>(null);
let unlisten: (() => void) | null = null;

/** 已安装版本所属大版本（用于行内标注） */
const installedMajor = computed(() => props.service.version?.split(".")[0] ?? "");

/** 已安装版本是否为某大版本最新（无更新则隐藏升级按钮） */
function isUpToDate(g: AvailableVersionGroup): boolean {
  return installedMajor.value === g.major && props.service.version === g.latest;
}

async function loadVersions() {
  if (!props.service.installed) {
    loadingVersions.value = true;
    try {
      versions.value = await availableServiceVersions(props.service.kind);
    } catch (e) {
      result.value = { ok: false, text: `获取版本列表失败: ${e}` };
    } finally {
      loadingVersions.value = false;
    }
  }
}

async function doInstall(target: string) {
  if (busy.value || !target) return;
  busy.value = true;
  result.value = null;
  progress.value = null;
  try {
    await installService(props.service.kind, target);
    result.value = { ok: true, text: `Redis ${target} 安装完成` };
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `安装失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

async function doStart() {
  if (busy.value || !props.service.version) return;
  busy.value = true;
  result.value = null;
  try {
    await startService(props.service.kind, props.service.version);
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `启动失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

async function doStop() {
  if (busy.value || !props.service.version) return;
  busy.value = true;
  result.value = null;
  try {
    await stopService(props.service.kind, props.service.version);
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `停止失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

async function doRestart() {
  if (busy.value || !props.service.version) return;
  busy.value = true;
  result.value = null;
  try {
    await restartService(props.service.kind, props.service.version);
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `重启失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

async function doUninstall() {
  if (busy.value || !props.service.version) return;
  if (!window.confirm("确定卸载 Redis？程序目录将被删除，数据目录保留。")) return;
  busy.value = true;
  result.value = null;
  try {
    await uninstallService(props.service.kind, props.service.version);
    result.value = { ok: true, text: "卸载成功（数据目录已保留）" };
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `卸载失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

onMounted(() => {
  loadVersions();
  onServiceProgress((p) => (progress.value = p)).then((fn) => (unlisten = fn));
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="svc">
    <div class="svc-head">
      <div class="svc-title">
        <span class="svc-icon">🗄️</span>
        <span>{{ service.name }}</span>
        <span
          v-if="service.installed"
          class="badge"
          :class="service.running ? 'badge-running' : 'badge-stopped'"
        >
          {{ service.running ? "运行中" : "已停止" }}
        </span>
        <span v-else class="badge">未安装</span>
      </div>
      <p v-if="service.note" class="svc-note">{{ service.note }}</p>
    </div>

    <!-- 状态卡 -->
    <div v-if="service.installed" class="svc-card">
      <div class="svc-row">
        <span class="svc-key">版本</span>
        <span class="svc-val">{{ service.version }}</span>
      </div>
      <div class="svc-row">
        <span class="svc-key">端口</span>
        <span class="svc-val">{{ service.port }}</span>
      </div>
      <div class="svc-row">
        <span class="svc-key">PID</span>
        <span class="svc-val">{{ service.pid ?? "—" }}</span>
      </div>
      <div class="svc-row">
        <span class="svc-key">数据目录</span>
        <span class="svc-val path">{{ service.dataDir }}</span>
      </div>

      <div class="svc-actions">
        <button
          v-if="!service.running"
          class="btn primary"
          :disabled="busy"
          @click="doStart"
        >
          启动
        </button>
        <template v-else>
          <button class="btn" :disabled="busy" @click="doStop">停止</button>
          <button class="btn" :disabled="busy" @click="doRestart">重启</button>
        </template>
        <button class="btn danger" :disabled="busy" @click="doUninstall">
          卸载
        </button>
      </div>
    </div>

    <!-- 安装面板：按大版本分组 -->
    <div v-else class="svc-card">
      <div class="svc-install">
        <p class="svc-install-tip">
          安装 {{ service.name }}（自动下载源码并编译，约 1-2 分钟）
        </p>
        <div v-if="loadingVersions" class="svc-muted">正在获取版本列表…</div>
        <div v-else-if="versions.length" class="svc-groups">
          <div v-for="g in versions" :key="g.major" class="svc-group-row">
            <span class="svc-major">{{ service.name }} {{ g.major }}</span>
            <span class="svc-val">{{ g.latest }}</span>
            <span
              v-if="installedMajor === g.major"
              class="badge"
              :class="isUpToDate(g) ? 'badge-stopped' : 'badge-upgrade'"
            >
              {{ isUpToDate(g) ? "已是最新" : "有新版本" }}
            </span>
            <button
              v-if="!isUpToDate(g)"
              class="btn primary small"
              :disabled="busy"
              @click="doInstall(g.latest)"
            >
              {{ installedMajor === g.major ? `升级到 ${g.latest}` : "安装" }}
            </button>
            <span v-else class="svc-muted">已安装 {{ service.version }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 安装进度 -->
    <div v-if="progress && progress.stage !== 'done'" class="svc-card">
      <div class="svc-row">
        <span class="svc-key">状态</span>
        <span class="svc-val">{{ progress.message }}</span>
      </div>
      <div class="bar">
        <div
          class="bar-fill"
          :class="{ indeterminate: progress.percent == null }"
          :style="progress.percent != null ? { width: `${progress.percent}%` } : {}"
        ></div>
      </div>
    </div>

    <div v-if="result" :class="['svc-result', result.ok ? 'ok' : 'err']">
      {{ result.text }}
    </div>
  </div>
</template>

<style scoped>
.svc {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 720px;
}

.svc-head {
  margin-bottom: 4px;
}

.svc-title {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 18px;
  font-weight: 600;
}

.svc-icon {
  font-size: 20px;
}

.badge {
  font-size: 12px;
  font-weight: 500;
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.badge-running {
  color: var(--success);
  border-color: var(--success);
  background: var(--success-soft);
}

.badge-stopped {
  color: var(--warning);
  border-color: var(--warning);
}

.svc-note {
  margin-top: 8px;
  font-size: 12px;
  color: var(--warning);
}

.svc-card {
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  background: var(--bg-panel);
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.svc-row {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 13px;
}

.svc-key {
  width: 76px;
  flex-shrink: 0;
  color: var(--text-secondary);
}

.svc-val {
  color: var(--text-primary);
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
}

.svc-val.path {
  word-break: break-all;
}

.svc-actions {
  display: flex;
  gap: 10px;
  margin-top: 6px;
}

.svc-install-tip {
  font-size: 13px;
  color: var(--text-secondary);
}

.svc-groups {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 4px;
}

.svc-group-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-app);
}

.svc-major {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  flex-shrink: 0;
}

.btn.small {
  padding: 5px 12px;
  font-size: 12px;
}

.badge-upgrade {
  color: var(--brand);
  border-color: var(--brand);
}

.svc-install-row {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 13px;
}

.svc-muted {
  font-size: 12px;
  color: var(--text-muted);
}

.bar {
  height: 8px;
  border-radius: 999px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  overflow: hidden;
}

.bar-fill {
  height: 100%;
  background: var(--brand);
  border-radius: 999px;
  transition: width 0.2s ease;
}

.bar-fill.indeterminate {
  width: 40%;
  animation: slide 1.2s ease-in-out infinite;
}

@keyframes slide {
  0% {
    transform: translateX(-120%);
  }
  100% {
    transform: translateX(320%);
  }
}

.svc-result {
  font-size: 13px;
  padding: 10px 14px;
  border-radius: 10px;
}

.svc-result.ok {
  color: var(--success);
  background: var(--success-soft);
  border: 1px solid var(--success);
}

.svc-result.err {
  color: var(--danger);
  background: var(--danger-soft);
  border: 1px solid var(--danger);
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 8px 18px;
  font-size: 13px;
}

.btn.primary {
  background: var(--brand);
  border-color: var(--brand);
  color: #fff;
}

.btn.primary:hover:not(:disabled) {
  background: var(--brand-hover);
}

.btn.danger {
  border-color: var(--danger);
  color: var(--danger);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
