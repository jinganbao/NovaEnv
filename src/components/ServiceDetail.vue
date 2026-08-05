<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  availableServiceVersions,
  installService,
  onServiceProgress,
  restartService,
  serviceLogs,
  setServiceAutostart,
  startService,
  stopService,
  uninstallService,
  updateServiceConfig,
} from "../api";
import type {
  AvailableVersionGroup,
  ServiceConfig,
  ServiceInfo,
  ServiceProgress,
} from "../types";
import { SERVICE_META } from "../types";

const props = defineProps<{ service: ServiceInfo }>();
const emit = defineEmits<{ (e: "refresh"): void }>();

const versions = ref<AvailableVersionGroup[]>([]);
/** 下拉选中的待安装版本 */
const installTarget = ref("");

/** 全部可用版本平铺（倒序，最新在前） */
const flatVersions = computed(() => {
  const flat: { version: string; installed: boolean }[] = [];
  for (const g of versions.value) {
    for (const v of g.versions) {
      flat.push({
        version: v,
        installed: !!props.service.versions.find((s) => s.version === v),
      });
    }
  }
  return flat.sort((a, b) => {
    const nums = (s: string) => s.match(/\d+/g)?.map(Number) ?? [];
    const na = nums(a.version);
    const nb = nums(b.version);
    const len = Math.max(na.length, nb.length);
    for (let i = 0; i < len; i++) {
      const x = na[i] ?? 0;
      const y = nb[i] ?? 0;
      if (x !== y) return y - x;
    }
    return 0;
  });
});
const loadingVersions = ref(false);
const busy = ref(false);
/** 正在执行的操作（版本 → 动作），用于按钮文案反馈 */
const acting = ref<{ version: string; action: "start" | "stop" | "restart" } | null>(null);
const result = ref<{ ok: boolean; text: string } | null>(null);
/** 视口内 Toast（固定右上角，任何滚动位置可见） */
const toast = ref<{ ok: boolean; text: string } | null>(null);
let toastTimer: number | undefined;
const progress = ref<ServiceProgress | null>(null);
let unlisten: (() => void) | null = null;

/** 显示操作反馈：toast 固定右上角（成功 2.5s 自动消失，失败 5s 常驻） */
function showToast(ok: boolean, text: string) {
  toast.value = { ok, text };
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (toast.value = null), ok ? 2500 : 5000);
}

/** 版本行「更多」展开状态 */
const expandedVer = ref("");

/** 本地乐观运行状态（操作成功立即翻转，避免全量 refresh 导致页面闪烁） */
const localRunning = ref<Record<string, boolean>>({});

/** 版本运行状态：优先本地乐观值，回退服务端扫描结果（service.versions[].running） */
function runningOf(version: string): boolean {
  const local = localRunning.value[version];
  if (local !== undefined) return local;
  const v = props.service.versions.find((x) => x.version === version);
  if (v) return v.running;
  return props.service.running;
}

/** 操作成功后延迟后台校准（等 toast 呈现后平滑刷新，消除视觉闪烁） */
function delayedRefresh(ms = 500) {
  window.setTimeout(() => emit("refresh"), ms);
}

// 安装配置（端口/密码）：默认端口取服务实际默认（redis 6379 / mysql 3306）
const installPort = ref(props.service.port || 6379);
const installPassword = ref("");
/** 密码框明文显示开关 */
const showPwdInstall = ref(false);
const showPwdEdit = ref(false);

// 修改配置弹窗（按版本）
const editOpen = ref(false);
const editVersion = ref("");
const editPort = ref(6379);
const editPassword = ref("");
/** MySQL 修改密码时的当前密码（用于认证） */
const editOldPassword = ref("");

// 日志弹窗
const logOpen = ref(false);
const logVersion = ref("");
const logContent = ref("");
const logLoading = ref(false);

/** 切换开机自启（launchd 托管：开机自启 + 崩溃自动拉起） */
async function toggleAutostart(version: string, autostart: boolean) {
  if (busy.value || !version) return;
  busy.value = true;
  result.value = null;
  try {
    await setServiceAutostart(props.service.kind, version, !autostart);
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `设置开机自启失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

/** 查看服务日志（尾部 200 行） */
async function openLog(version: string) {
  if (!version) return;
  logVersion.value = version;
  logOpen.value = true;
  logLoading.value = true;
  logContent.value = "";
  try {
    logContent.value = await serviceLogs(props.service.kind, version);
  } catch (e) {
    logContent.value = `读取日志失败: ${e}`;
  } finally {
    logLoading.value = false;
  }
}

async function refreshLog() {
  if (!logVersion.value) return;
  logLoading.value = true;
  try {
    logContent.value = await serviceLogs(props.service.kind, logVersion.value);
  } catch (e) {
    logContent.value = `读取日志失败: ${e}`;
  } finally {
    logLoading.value = false;
  }
}

/** 已安装版本所属大版本（用于行内标注） */
/** 已安装版本列表（逗号分隔展示） */
const installedText = computed(() =>
  props.service.versions.map((v) => v.version).join("、"),
);

/** 版本列表会话级缓存（切回同服务不重复网络拉取，消除切换卡顿） */
const versionsCache = new Map<string, { at: number; groups: AvailableVersionGroup[] }>();

/** 该大版本是否已装有最新版本（无更新则隐藏升级按钮） */
async function loadVersions() {
  const key = props.service.kind;
  const hit = versionsCache.get(key);
  if (hit && Date.now() - hit.at < 5 * 60_000) {
    versions.value = hit.groups;
    return;
  }
  loadingVersions.value = true;
  try {
    versions.value = await availableServiceVersions(props.service.kind);
    versionsCache.set(key, { at: Date.now(), groups: versions.value });
  } catch (e) {
    // 拉取失败不阻塞界面（保留缓存旧值或空态）
    result.value = { ok: false, text: `获取版本列表失败: ${e}` };
  } finally {
    loadingVersions.value = false;
  }
}

async function doInstall() {
  const target = installTarget.value;
  if (busy.value || !target) return;
  busy.value = true;
  result.value = null;
  progress.value = null;
  try {
    await installService(props.service.kind, target, {
      port: installPort.value,
      password: installPassword.value.trim(),
    });
    showToast(true, `${props.service.name} ${target} 安装完成`);
    emit("refresh");
  } catch (e) {
    showToast(false, `安装失败: ${e}`);
  } finally {
    busy.value = false;
  }
}

/** 打开修改配置弹窗（预填该版本当前端口；密码留空表示不修改） */
function openEdit(version: string, port: number) {
  editVersion.value = version;
  editPort.value = port;
  editPassword.value = "";
  editOldPassword.value = "";
  editOpen.value = true;
}

/** 保存配置修改；运行中自动重启生效 */
async function saveEdit() {
  if (busy.value || !editVersion.value) return;
  busy.value = true;
  result.value = null;
  try {
    const config: ServiceConfig = {
      port: editPort.value,
      password: editPassword.value.trim(),
      oldPassword: editOldPassword.value.trim(),
    };
    await updateServiceConfig(props.service.kind, editVersion.value, config);
    editOpen.value = false;
    result.value = { ok: true, text: "配置已保存" + (props.service.running ? "，服务已自动重启生效" : "") };
    emit("refresh");
  } catch (e) {
    result.value = { ok: false, text: `保存失败: ${e}` };
  } finally {
    busy.value = false;
  }
}

async function doStart(version: string) {
  if (busy.value || !version) return;
  busy.value = true;
  acting.value = { version, action: "start" };
  result.value = null;
  try {
    await startService(props.service.kind, version);
    localRunning.value = { ...localRunning.value, [version]: true };
    showToast(true, `${props.service.name} ${version} 已启动`);
    delayedRefresh();
  } catch (e) {
    showToast(false, `启动失败: ${e}`);
  } finally {
    busy.value = false;
    acting.value = null;
  }
}

async function doStop(version: string) {
  if (busy.value || !version) return;
  busy.value = true;
  acting.value = { version, action: "stop" };
  result.value = null;
  try {
    await stopService(props.service.kind, version);
    localRunning.value = { ...localRunning.value, [version]: false };
    showToast(true, `${props.service.name} ${version} 已停止`);
    delayedRefresh();
  } catch (e) {
    showToast(false, `停止失败: ${e}`);
  } finally {
    busy.value = false;
    acting.value = null;
  }
}

async function doRestart(version: string) {
  if (busy.value || !version) return;
  busy.value = true;
  acting.value = { version, action: "restart" };
  result.value = null;
  try {
    await restartService(props.service.kind, version);
    showToast(true, `${props.service.name} ${version} 已重启`);
    delayedRefresh();
  } catch (e) {
    showToast(false, `重启失败: ${e}`);
  } finally {
    busy.value = false;
    acting.value = null;
  }
}

async function doUninstall(version: string) {
  if (busy.value || !version) return;
  if (!window.confirm(`确定卸载 ${props.service.name} ${version}？程序目录将被删除，数据目录保留。`)) return;
  busy.value = true;
  result.value = null;
  try {
    await uninstallService(props.service.kind, version);
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
    <!-- 操作反馈 Toast（固定右上角，视口内始终可见） -->
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

    <div class="page-head">
      <span
        class="head-icon"
        :style="{ background: SERVICE_META[service.kind].color }"
        >{{ SERVICE_META[service.kind].letter }}</span
      >
      <div class="head-text">
        <h2>{{ service.name }}</h2>
        <p>服务类组件 · 支持多版本并存</p>
      </div>
      <div class="head-status">
        <span
          v-if="service.installed"
          class="badge"
          :class="runningOf(service.version ?? '') ? 'badge-success' : 'badge-warning'"
        >
          <span class="dot" :class="runningOf(service.version ?? '') ? 'dot-on' : 'dot-off'"></span>
          {{ runningOf(service.version ?? '') ? "运行中" : "已停止" }}
        </span>
        <span v-else class="badge">未安装</span>
      </div>
    </div>
    <p v-if="service.note" class="svc-note">{{ service.note }}</p>

    <!-- 安装面板：版本下拉 + 端口/密码（始终可见） -->
    <div class="svc-card install-panel">
      <div class="svc-install">
        <div class="install-head">
          <p class="svc-install-tip">
            {{
              service.installed
                ? `安装其他版本（当前已安装：${installedText}）`
                : `安装 ${service.name}（自动下载并安装，约 1-2 分钟）`
            }}
          </p>
        </div>
        <div class="install-row">
          <label class="cfg-field">
            <span class="cfg-label">端口</span>
            <input
              v-model.number="installPort"
              type="number"
              min="1"
              max="65535"
              class="cfg-input"
            />
          </label>
          <label class="cfg-field">
            <span class="cfg-label">密码（可选）</span>
            <div class="pwd-wrap">
              <input
                v-model="installPassword"
                :type="showPwdInstall ? 'text' : 'password'"
                placeholder="留空则不设置密码"
                class="cfg-input"
              />
              <button
                class="pwd-toggle"
                type="button"
                :title="showPwdInstall ? '隐藏密码' : '显示明文'"
                @click="showPwdInstall = !showPwdInstall"
              >
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path v-if="showPwdInstall" d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                  <path v-else d="M1 1l22 22M10.58 10.58a2 2 0 0 0 2.83 2.83" />
                </svg>
              </button>
            </div>
          </label>
          <label class="cfg-field cfg-version">
            <span class="cfg-label">版本</span>
            <select v-model="installTarget" class="cfg-input cfg-select" :disabled="busy || loadingVersions">
              <option value="" disabled>选择版本…</option>
              <option
                v-for="ver in flatVersions"
                :key="ver.version"
                :value="ver.version"
                :disabled="ver.installed"
              >
                {{ ver.version }}{{ ver.installed ? "（已安装）" : "" }}
              </option>
            </select>
          </label>
          <button
            class="btn btn-primary"
            :disabled="busy || !installTarget"
            @click="doInstall()"
          >
            {{ busy ? "安装中…" : "安装" }}
          </button>
        </div>
        <div v-if="loadingVersions" class="svc-muted install-loading">正在获取版本列表…</div>
      </div>
    </div>

    <!-- 已安装版本（表格形式） -->
    <div v-if="service.installed" class="svc-card">
      <div class="installed-head">
        <span class="installed-title">已安装版本</span>
        <span class="svc-muted">{{ service.versions.length }} 个</span>
      </div>
      <div class="ver-table">
        <div class="ver-table-head">
          <span class="col-ver">版本</span>
          <span class="col-port">端口</span>
          <span class="col-status">状态</span>
          <span class="col-auto">自启</span>
          <span class="col-ops">操作</span>
        </div>
        <div
          v-for="v in service.versions"
          :key="v.version"
          class="ver-table-row"
          :class="{
            'row-running': runningOf(v.version),
            'row-expanded': expandedVer === v.version,
          }"
        >
          <span class="col-ver ver-name">{{ v.version }}</span>
          <span class="col-port">{{ v.port }}</span>
          <span class="col-status">
            <span
              class="run-dot"
              :class="runningOf(v.version) ? 'run-dot-on' : 'run-dot-off'"
            ></span>
            <span :class="runningOf(v.version) ? 'status-running' : 'status-stopped'">
              {{ runningOf(v.version) ? "运行中" : "已停止" }}
            </span>
          </span>
          <span class="col-auto">
            <span v-if="v.autostart" class="badge badge-brand badge-tiny">自启</span>
            <span v-else class="svc-muted">—</span>
          </span>
          <span class="col-ops">
            <button
              v-if="!runningOf(v.version)"
              class="btn btn-primary btn-sm"
              :disabled="busy"
              @click="doStart(v.version)"
            >
              {{ acting?.version === v.version && acting?.action === 'start' ? '启动中…' : '启动' }}
            </button>
            <button
              v-else
              class="btn btn-sm"
              :disabled="busy"
              @click="doStop(v.version)"
            >
              {{ acting?.version === v.version && acting?.action === 'stop' ? '停止中…' : '停止' }}
            </button>
            <button
              class="btn btn-sm btn-more"
              :class="{ open: expandedVer === v.version }"
              :disabled="busy"
              @click="expandedVer = expandedVer === v.version ? '' : v.version"
            >
              更多
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path :d="expandedVer === v.version ? 'M18 15l-6-6-6 6' : 'M6 9l6 6 6-6'" />
              </svg>
            </button>
          </span>

          <!-- 更多操作（展开行） -->
          <div v-if="expandedVer === v.version" class="ver-more">
            <button class="btn btn-sm" :disabled="busy" @click="doRestart(v.version)">
              {{ acting?.version === v.version && acting?.action === 'restart' ? '重启中…' : '重启' }}
            </button>
            <button class="btn btn-sm" :disabled="busy" @click="openEdit(v.version, v.port)">
              配置
            </button>
            <button class="btn btn-sm" :disabled="busy" @click="openLog(v.version)">
              日志
            </button>
            <button
              class="btn btn-sm"
              :disabled="busy"
              @click="toggleAutostart(v.version, v.autostart)"
            >
              {{ v.autostart ? "取消自启" : "开机自启" }}
            </button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="doUninstall(v.version)">
              卸载
            </button>
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

    <!-- 修改配置弹窗 -->
    <div v-if="editOpen" class="modal-mask" @click.self="!busy && (editOpen = false)">
      <div class="modal">
        <h3>修改 {{ service.name }} 配置</h3>
        <p class="modal-sub">运行中保存后自动重启生效</p>
        <div class="cfg-field">
          <span class="cfg-label">端口</span>
          <input v-model.number="editPort" type="number" min="1" max="65535" class="cfg-input" />
        </div>
        <div v-if="service.kind === 'mysql'" class="cfg-field">
          <span class="cfg-label">当前密码</span>
          <input
            v-model="editOldPassword"
            :type="showPwdEdit ? 'text' : 'password'"
            placeholder="修改密码时用于认证"
            class="cfg-input"
          />
        </div>
        <div class="cfg-field">
          <span class="cfg-label">新密码</span>
          <div class="pwd-wrap">
            <input
              v-model="editPassword"
              :type="showPwdEdit ? 'text' : 'password'"
              placeholder="留空表示不修改密码"
              class="cfg-input"
            />
            <button
              class="pwd-toggle"
              type="button"
              :title="showPwdEdit ? '隐藏密码' : '显示明文'"
              @click="showPwdEdit = !showPwdEdit"
            >
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path v-if="showPwdEdit" d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                <path v-else d="M1 1l22 22M10.58 10.58a2 2 0 0 0 2.83 2.83" />
              </svg>
            </button>
          </div>
        </div>
        <div class="actions">
          <button class="btn" :disabled="busy" @click="editOpen = false">取消</button>
          <button class="btn primary" :disabled="busy" @click="saveEdit">
            {{ busy ? "保存中…" : "保存" }}
          </button>
        </div>
      </div>
    </div>

    <!-- 日志弹窗 -->
    <div v-if="logOpen" class="modal-mask" @click.self="logOpen = false">
      <div class="modal modal-log">
        <div class="log-head">
          <h3>{{ service.name }} 日志</h3>
          <button class="btn small" :disabled="logLoading" @click="refreshLog">
            {{ logLoading ? "加载中…" : "刷新" }}
          </button>
        </div>
        <pre class="log-body">{{ logContent || (logLoading ? "加载中…" : "暂无日志内容") }}</pre>
        <div class="actions">
          <button class="btn" @click="logOpen = false">关闭</button>
        </div>
      </div>
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

/* 安装/修改配置输入 */
.svc-config {
  display: flex;
  gap: 14px;
  margin-top: 4px;
  padding-top: 10px;
  border-top: 1px dashed var(--border-subtle);
}

.cfg-field {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cfg-label {
  font-size: 12px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.cfg-input {
  width: 130px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-primary);
  padding: 6px 10px;
  font-size: 13px;
}

.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 100;
}

.modal {
  width: min(400px, 92vw);
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  padding: 22px 24px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.modal h3 {
  font-size: 16px;
}

.modal-sub {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: -8px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 6px;
}

/* 开机自启开关 */
.switch {
  width: 38px;
  height: 22px;
  border-radius: 999px;
  border: 1px solid var(--border-strong);
  background: var(--bg-input);
  position: relative;
  transition: background 0.2s, border-color 0.2s;
  padding: 0;
}

.switch.on {
  background: var(--brand);
  border-color: var(--brand);
}

.switch-dot {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #fff;
  transition: transform 0.2s;
}

.switch.on .switch-dot {
  transform: translateX(16px);
}

.btn.small {
  padding: 4px 12px;
  font-size: 12px;
}

/* 日志弹窗 */
.modal-log {
  width: min(720px, 94vw);
}

.log-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.log-body {
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 12px 14px;
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-secondary);
  max-height: 50vh;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
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

/* 多版本列表行 */
.svc-version-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-subtle, #262c35);
  flex-wrap: wrap;
}
.svc-version-row:last-of-type {
  border-bottom: none;
}
.svc-version-row .version {
  font-weight: 600;
  min-width: 72px;
}
.badge-autostart {
  background: rgba(52, 211, 153, 0.12);
  color: var(--brand, #34d399);
}
.svc-actions {
  display: flex;
  gap: 6px;
  margin-left: auto;
  flex-wrap: wrap;
}
.btn.small {
  padding: 4px 10px;
  font-size: 12px;
}

/* ---- 页面头（与服务端统一） ---- */
.page-head {
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

.head-status {
  margin-left: auto;
}

/* 顶部状态徽章：更醒目（加粗 + 浓色底 + 实边框） */
.head-status .badge {
  padding: 5px 14px;
  font-size: 13px;
  font-weight: 600;
  border-radius: 99px;
  display: inline-flex;
  align-items: center;
  gap: 7px;
}

.head-status .badge-success {
  background: rgba(74, 222, 128, 0.16);
  border: 1px solid rgba(74, 222, 128, 0.5);
  color: var(--success);
  box-shadow: 0 0 0 3px rgba(74, 222, 128, 0.08);
}

.head-status .badge-warning {
  background: rgba(251, 191, 36, 0.14);
  border: 1px solid rgba(251, 191, 36, 0.45);
  color: var(--warning);
}

.head-status .dot {
  width: 8px;
  height: 8px;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.dot-on {
  background: currentColor;
  animation: status-pulse 1.6s ease-in-out infinite;
}

.dot-off {
  background: currentColor;
}

/* 运行状态脉冲动画 */
@keyframes status-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

/* 操作反馈 Toast */
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

/* ---- 安装面板（列表上方） ---- */
.install-panel {
  border-color: var(--border-strong);
}

.install-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  flex-wrap: wrap;
}

.svc-config {
  display: flex;
  gap: var(--space-3);
  align-items: flex-end;
}

/* ---- 已安装版本列表 ---- */
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

.run-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  transition: background var(--duration) var(--ease), box-shadow var(--duration) var(--ease);
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

.btn-more {
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.btn-more.open {
  color: var(--brand);
  border-color: var(--brand);
}

/* 更多操作展开条 */
.ver-more {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 16px 12px 42px;
  border-top: 1px dashed var(--border-subtle);
  background: var(--bg-app);
  flex-wrap: wrap;
}

/* ---- 安装面板（下拉行） ---- */
.install-row {
  display: flex;
  align-items: flex-end;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.cfg-version {
  min-width: 220px;
  flex: 1;
}

.cfg-select {
  appearance: none;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%239aa6b7' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'><path d='M6 9l6 6 6-6'/></svg>");
  background-repeat: no-repeat;
  background-position: right 10px center;
  padding-right: 30px;
  cursor: pointer;
}

.cfg-select:disabled {
  cursor: not-allowed;
}

.install-loading {
  margin-top: var(--space-2);
  font-size: var(--text-sm);
}

/* ---- 已安装版本表格 ---- */
.ver-table {
  display: flex;
  flex-direction: column;
}

.ver-table-head {
  display: grid;
  grid-template-columns: 120px 80px 110px 70px 1fr;
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
  grid-template-columns: 120px 80px 110px 70px 1fr;
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

.ver-table-row.row-expanded {
  background: var(--bg-app);
}

/* 运行中的版本行：绿色微亮 + 左侧标记 */
.ver-table-row.row-running {
  background: var(--success-soft);
}

.ver-table-row.row-running:hover {
  background: var(--success-soft);
}

/* 状态文字：运行中绿色加粗，已停止灰色 */
.status-running {
  color: var(--success);
  font-weight: 600;
}

.status-stopped {
  color: var(--text-muted);
}

.run-dot-on {
  animation: status-pulse 1.6s ease-in-out infinite;
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

/* 更多操作展开行（跨整行） */
.ver-more {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 0 4px;
  border-top: 1px dashed var(--border-subtle);
  margin-top: 2px;
  flex-wrap: wrap;
}

/* 密码框 + 明文切换 */
.pwd-wrap {
  position: relative;
  display: flex;
  align-items: center;
}

.pwd-wrap .cfg-input {
  padding-right: 36px;
  width: 100%;
}

.pwd-toggle {
  position: absolute;
  right: 4px;
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  border-radius: var(--radius-sm);
  transition: color var(--duration) var(--ease), background var(--duration) var(--ease);
}

.pwd-toggle:hover {
  color: var(--text-primary);
  background: var(--bg-panel-hover);
}
</style>
