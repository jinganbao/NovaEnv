<script setup lang="ts">
import type { RuntimeVersion } from "../types";

defineProps<{ version: RuntimeVersion }>();
const emit = defineEmits<{
  (e: "activate", version: RuntimeVersion): void;
  (e: "uninstall", version: RuntimeVersion): void;
}>();
</script>

<template>
  <div class="card" :class="{ 'is-default': version.isDefault }">
    <div class="card-main">
      <div class="card-title">
        <span class="ver">{{ version.version }}</span>
        <span v-if="version.isDefault" class="badge default">当前默认</span>
        <span v-if="version.managed" class="badge managed">NovaEnv</span>
      </div>
      <div class="vendor">{{ version.vendor }}</div>
      <div class="path" :title="version.path">{{ version.path }}</div>
    </div>
    <div class="actions">
      <button
        class="btn"
        :disabled="version.isDefault"
        @click="emit('activate', version)"
      >
        {{ version.isDefault ? "默认中" : "设为默认" }}
      </button>
      <button
        v-if="version.managed && !version.isDefault"
        class="btn danger"
        @click="emit('uninstall', version)"
      >
        卸载
      </button>
    </div>
  </div>
</template>

<style scoped>
.card {
  display: flex;
  align-items: center;
  gap: 14px;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  padding: 12px 16px;
  background: var(--bg-app);
  transition: border-color 0.15s;
}

.card.is-default {
  border-color: var(--success);
  background: var(--success-soft);
}

.card-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.card-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ver {
  font-size: 15px;
  font-weight: 700;
}

.badge {
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 999px;
  color: #fff;
}

.badge.default {
  background: var(--success);
}

.badge.managed {
  background: var(--brand);
}

.vendor {
  color: var(--text-secondary);
  font-size: 12px;
}

.path {
  color: var(--text-secondary);
  font-family: "SF Mono", Menlo, Consolas, monospace;
  font-size: 11px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
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

.btn:hover:not(:disabled) {
  background: var(--bg-panel-hover);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.danger {
  color: var(--danger);
  border-color: var(--danger);
}

.btn.danger:hover:not(:disabled) {
  background: var(--danger-soft);
}
</style>
