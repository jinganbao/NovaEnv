<script setup lang="ts">
import type { RuntimeVersion } from "../types";

defineProps<{ version: RuntimeVersion; busy: boolean }>();
const emit = defineEmits<{ (e: "confirm"): void; (e: "cancel"): void }>();
</script>

<template>
  <div class="modal-mask" @click.self="!busy && emit('cancel')">
    <div class="modal">
      <h3>卸载版本</h3>
      <p class="sub">
        确认卸载 <strong>{{ version.vendor }} {{ version.version }}</strong>？
      </p>
      <div class="path" :title="version.path">{{ version.path }}</div>
      <p class="note">仅删除 NovaEnv 管理的安装目录，不影响系统其他安装。</p>
      <div class="actions">
        <button class="btn" :disabled="busy" @click="emit('cancel')">取消</button>
        <button class="btn danger" :disabled="busy" @click="emit('confirm')">
          {{ busy ? "卸载中…" : "确认卸载" }}
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
  width: min(440px, 92vw);
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

.sub {
  color: var(--text-secondary);
  font-size: 13px;
}

.path {
  margin-top: 8px;
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.note {
  margin-top: 10px;
  font-size: 12px;
  color: var(--warning);
}

.actions {
  margin-top: 18px;
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

.btn.danger {
  color: #fff;
  background: var(--danger);
  border-color: var(--danger);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
