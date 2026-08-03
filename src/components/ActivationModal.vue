<script setup lang="ts">
import type { ActivationPreview, RuntimeVersion } from "../types";

defineProps<{
  version: RuntimeVersion;
  preview: ActivationPreview;
  busy: boolean;
}>();
const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();
</script>

<template>
  <div class="modal-mask" @click.self="!busy && emit('cancel')">
    <div class="modal">
      <h3>切换默认版本</h3>
      <p class="modal-sub">
        将 <strong>{{ version.vendor }} {{ version.version }}</strong> 设为默认
      </p>
      <div class="path-inline" :title="version.path">{{ version.path }}</div>

      <div class="preview">
        <div v-if="preview.configFile" class="pv-line">
          <span class="pv-key">配置文件</span>
          <code>{{ preview.configFile }}</code>
        </div>
        <pre class="pv-script">{{ preview.lines.join("\n") }}</pre>
        <div v-if="preview.backupPath" class="pv-line">
          <span class="pv-key">备份</span>
          <code>{{ preview.backupPath }}</code>
        </div>
      </div>

      <p class="pv-note">{{ preview.note }}</p>

      <div class="actions">
        <button class="btn" :disabled="busy" @click="emit('cancel')">取消</button>
        <button class="btn primary" :disabled="busy" @click="emit('confirm')">
          {{ busy ? "执行中…" : "确认切换" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 100;
}

.modal {
  width: min(560px, 92vw);
  max-height: 80vh;
  overflow-y: auto;
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  padding: 22px 24px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
}

h3 {
  font-size: 17px;
  margin-bottom: 8px;
}

.modal-sub {
  color: var(--text-secondary);
  font-size: 13px;
}

.path-inline {
  margin-top: 6px;
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.preview {
  margin: 16px 0 10px;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-app);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pv-line {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-secondary);
}

.pv-line code {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pv-script {
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 12px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
  -webkit-user-select: text;
}

.pv-note {
  color: var(--warning);
  font-size: 12px;
  margin-bottom: 16px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
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

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
