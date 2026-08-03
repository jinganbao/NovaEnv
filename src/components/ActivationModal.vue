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
      <p class="modal-hint">
        将更新
        <code>{{ preview.configFile || "系统环境变量" }}</code>
        ，新打开的终端自动生效。
      </p>
      <div class="actions">
        <button class="btn" :disabled="busy" @click="emit('cancel')">取消</button>
        <button class="btn primary" :disabled="busy" @click="emit('confirm')">
          {{ busy ? "执行中…" : "确认" }}
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

.modal-sub {
  color: var(--text-secondary);
  font-size: 13px;
}

.modal-hint {
  margin: 14px 0 20px;
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.7;
}

.modal-hint code {
  color: var(--text-primary);
  word-break: break-all;
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
