<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { visionLogs, visionStart, visionStatus, visionStop } from "../api";
import { VISION_META } from "../types";
import type { VisionInfo } from "../types";

/** 接入配置示例（模板用） */
const mcpJson = `{
  "mcpServers": {
    "vision": {
      "command": "python3",
      "args": ["~/.novaenv/vision-mcp/server.py"],
      "env": { "ZHIPU_API_KEY": "<你的 Key>" }
    }
  }
}`;

const info = ref<VisionInfo | null>(null);
const busy = ref(false);
const logOpen = ref(false);
const logContent = ref("");
const logLoading = ref(false);

/** API Key（localStorage 持久化，与配置机制一致） */
const KEY_STORAGE = "NovaEnv-vision-key";
const apiKey = ref(localStorage.getItem(KEY_STORAGE) ?? "");
const keyDirty = ref(false);
const keySaved = ref(false);

const result = ref<{ ok: boolean; text: string } | null>(null);
let pollTimer: number | undefined;
let resultTimer: number | undefined;

const running = computed(() => info.value?.running ?? false);
const pythonOk = computed(() => info.value?.python != null);
const depsReady = computed(() => info.value?.depsReady ?? false);

function showResult(ok: boolean, text: string) {
  result.value = { ok, text };
  window.clearTimeout(resultTimer);
  if (ok) resultTimer = window.setTimeout(() => (result.value = null), 4000);
}

async function refresh() {
  try {
    info.value = await visionStatus();
  } catch (e) {
    showResult(false, `获取状态失败: ${e}`);
  }
}

function onKeyInput() {
  keyDirty.value = true;
  keySaved.value = false;
}

function saveKey() {
  localStorage.setItem(KEY_STORAGE, apiKey.value.trim());
  keyDirty.value = false;
  keySaved.value = true;
  showResult(true, "API Key 已保存");
  window.setTimeout(() => (keySaved.value = false), 2000);
}

async function doStart() {
  if (busy.value) return;
  busy.value = true;
  result.value = null;
  try {
    if (keyDirty.value) saveKey();
    if (!apiKey.value.trim()) {
      showResult(false, "请先填写智谱 API Key");
      return;
    }
    await visionStart(apiKey.value.trim());
    showResult(true, "Vision MCP 服务已启动（首次启动会自动安装依赖，可能需要几分钟）");
    setTimeout(refresh, 1500);
  } catch (e) {
    showResult(false, `启动失败: ${e}`);
  } finally {
    busy.value = false;
  }
}

async function doStop() {
  if (busy.value) return;
  busy.value = true;
  try {
    await visionStop();
    showResult(true, "服务已停止");
    refresh();
  } catch (e) {
    showResult(false, `停止失败: ${e}`);
  } finally {
    busy.value = false;
  }
}

async function openLog() {
  logOpen.value = true;
  logLoading.value = true;
  logContent.value = "";
  try {
    logContent.value = await visionLogs();
  } catch (e) {
    logContent.value = `读取日志失败: ${e}`;
  } finally {
    logLoading.value = false;
  }
}

onMounted(() => {
  refresh();
  pollTimer = window.setInterval(refresh, 3000);
});
onUnmounted(() => window.clearInterval(pollTimer));
</script>

<template>
  <div class="page">
    <div class="page-head">
      <span class="head-icon" :style="{ background: VISION_META.color }">{{ VISION_META.letter }}</span>
      <div class="head-text">
        <h2>{{ VISION_META.name }}</h2>
        <p>{{ VISION_META.desc }}</p>
      </div>
      <div class="head-status">
        <span v-if="running" class="badge badge-success">
          <span class="dot dot-on"></span>运行中
        </span>
        <span v-else class="badge badge-warning"><span class="dot dot-off"></span>已停止</span>
      </div>
    </div>

    <div v-if="result" class="panel-result" :class="result.ok ? 'ok' : 'err'">
      {{ result.text }}
    </div>

    <!-- 运行信息 -->
    <section class="card">
      <h3>服务信息</h3>
      <div class="row">
        <span class="row-label">状态</span>
        <span class="row-value">
          <span class="status-text" :class="running ? 'status-running' : 'status-stopped'">
            {{ running ? "运行中" : "已停止" }}
          </span>
          <span v-if="info?.pid" class="row-hint">PID {{ info.pid }}</span>
        </span>
      </div>
      <div class="row">
        <span class="row-label">Python</span>
        <span class="row-value">
          {{ pythonOk ? "python3 已找到" : "未找到 python3（需 Python 3.10+）" }}
        </span>
      </div>
      <div class="row">
        <span class="row-label">依赖</span>
        <span class="row-value">
          <span v-if="depsReady" class="status-text status-running">已就绪</span>
          <span v-else class="status-text status-stopped">未安装（启动时自动安装）</span>
        </span>
      </div>
      <div class="row">
        <span class="row-label">日志文件</span>
        <code class="row-value path">{{ info?.logFile ?? "—" }}</code>
      </div>
      <div class="row">
        <span class="row-label">模型</span>
        <span class="row-value">glm-4.6v-flash（智谱多模态）</span>
      </div>
      <div class="row actions-row">
        <button v-if="!running" class="btn btn-primary" :disabled="busy" @click="doStart">
          {{ busy ? "启动中…" : "启动" }}
        </button>
        <button v-else class="btn" :disabled="busy" @click="doStop">
          {{ busy ? "停止中…" : "停止" }}
        </button>
        <button class="btn" @click="openLog">查看日志</button>
      </div>
    </section>

    <!-- API Key -->
    <section class="card">
      <h3>智谱 API Key</h3>
      <div class="row">
        <span class="row-label">Key</span>
        <div class="key-input-wrap">
          <input
            class="key-input"
            type="password"
            placeholder="sk- 或 a5...（智谱开放平台申请）"
            v-model="apiKey"
            @input="onKeyInput"
          />
          <button class="btn btn-primary btn-sm" :disabled="!keyDirty" @click="saveKey">
            {{ keySaved ? "已保存 ✓" : "保存" }}
          </button>
        </div>
      </div>
      <p class="hint">Key 保存在本机（localStorage），仅用于启动服务时注入环境变量，不会上传。</p>
    </section>

    <!-- 接入说明 -->
    <section class="card">
      <h3>接入 Reasonix</h3>
      <p class="hint">在 Reasonix 项目根目录创建 <code>.mcp.json</code>：</p>
      <pre class="code-block">{{ mcpJson }}</pre>
      <p class="hint">
        重启 Reasonix 后，粘贴图片即可让模型自动调用
        <code>analyze_image</code> / <code>ocr_image</code> / <code>describe_image</code> 识别。
      </p>
    </section>

    <!-- 日志弹窗 -->
    <Teleport to="body">
      <div v-if="logOpen" class="log-mask" @click.self="logOpen = false">
        <div class="log-card">
          <div class="log-head">
            <h3>Vision MCP 日志</h3>
            <button class="btn btn-sm" @click="logOpen = false">关闭</button>
          </div>
          <pre class="log-body">{{ logLoading ? "加载中…" : logContent || "（暂无日志）" }}</pre>
        </div>
      </div>
    </Teleport>
  </div>
</template>


<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.page-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding-bottom: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.head-icon {
  width: 42px;
  height: 42px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  color: #fff;
  font-weight: 700;
  font-size: 18px;
}

.head-text h2 {
  font-size: 18px;
  margin: 0;
}

.head-text p {
  font-size: 12.5px;
  color: var(--text-secondary);
  margin: 2px 0 0;
}

.head-status {
  margin-left: auto;
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 5px 14px;
  font-size: 13px;
  font-weight: 600;
  border-radius: 99px;
}

.badge-success {
  background: rgba(74, 222, 128, 0.16);
  border: 1px solid rgba(74, 222, 128, 0.5);
  color: var(--success);
}

.badge-warning {
  background: rgba(251, 191, 36, 0.14);
  border: 1px solid rgba(251, 191, 36, 0.45);
  color: var(--warning);
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.dot-on {
  background: currentColor;
  animation: vpulse 1.6s ease-in-out infinite;
}

@keyframes vpulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

.panel-result {
  padding: 10px 14px;
  border-radius: 8px;
  font-size: 13px;
}

.panel-result.ok {
  background: var(--success-soft);
  color: var(--success);
}

.panel-result.err {
  background: var(--danger-soft);
  color: var(--danger);
}

.card {
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  padding: 16px 18px;
}

.card h3 {
  font-size: 14.5px;
  margin: 0 0 10px;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 0;
  font-size: 13px;
}

.row-label {
  color: var(--text-muted);
  width: 72px;
  flex-shrink: 0;
}

.row-value {
  color: var(--text-primary);
}

.row-hint {
  color: var(--text-muted);
  font-size: 12px;
  margin-left: 8px;
}

.path {
  font-family: "SF Mono", Menlo, monospace;
  font-size: 12px;
  color: var(--text-secondary);
  word-break: break-all;
}

.status-running {
  color: var(--success);
  font-weight: 600;
}

.status-stopped {
  color: var(--text-muted);
}

.actions-row {
  margin-top: 6px;
  gap: 10px;
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel-hover);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 8px 18px;
  font-size: 13px;
  cursor: pointer;
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.btn-primary {
  background: var(--brand);
  border-color: var(--brand);
  color: #08130d;
  font-weight: 600;
}

.btn-sm {
  padding: 6px 14px;
}

.key-input-wrap {
  display: flex;
  gap: 8px;
  align-items: center;
  flex: 1;
}

.key-input {
  flex: 1;
  min-width: 0;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-primary);
  padding: 8px 12px;
  font-size: 13px;
  font-family: "SF Mono", Menlo, monospace;
}

.hint {
  color: var(--text-muted);
  font-size: 12.5px;
  line-height: 1.7;
  margin: 6px 0 0;
}

.hint code {
  color: var(--text-primary);
  background: var(--bg-input);
  padding: 1px 5px;
  border-radius: 4px;
}

.code-block {
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 12px 14px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-secondary);
  overflow-x: auto;
  margin: 8px 0 0;
  font-family: "SF Mono", Menlo, monospace;
}

.log-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 100;
  backdrop-filter: blur(2px);
}

.log-card {
  width: min(640px, 92vw);
  max-height: 80vh;
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.log-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.log-head h3 {
  margin: 0;
  font-size: 14.5px;
}

.log-body {
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 12px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-secondary);
  max-height: 55vh;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: "SF Mono", Menlo, monospace;
  margin: 0;
}
</style>
