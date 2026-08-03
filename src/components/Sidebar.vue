<script setup lang="ts">
import type { RuntimeKind } from "../types";
import { RUNTIME_META } from "../types";

defineProps<{
  kinds: RuntimeKind[];
  selected: RuntimeKind | "settings";
  counts: Record<RuntimeKind, number>;
}>();
const emit = defineEmits<{
  (e: "select", kind: RuntimeKind | "settings"): void;
}>();

function isKind(value: RuntimeKind | "settings"): value is RuntimeKind {
  return value !== "settings";
}
</script>

<template>
  <aside class="sidebar">
    <nav class="nav">
      <button
        v-for="k in kinds"
        :key="k"
        class="nav-item"
        :class="{ active: selected === k }"
        @click="emit('select', k)"
      >
        <span class="nav-icon">{{ RUNTIME_META[k].icon }}</span>
        <span class="nav-name">{{ RUNTIME_META[k].name }}</span>
        <span class="nav-count">{{ counts[k] }}</span>
      </button>

      <button
        class="nav-item"
        :class="{ active: selected === 'settings' }"
        @click="emit('select', 'settings')"
      >
        <span class="nav-icon">⚙️</span>
        <span class="nav-name">设置</span>
        <span v-if="!isKind(selected)" class="nav-dot"></span>
      </button>
    </nav>
    <div class="sidebar-foot">
      <span class="foot-label">管理目录</span>
      <code class="foot-path">~/.novaenv</code>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 180px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  border-right: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 14px 10px;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-primary);
  border-radius: 10px;
  padding: 10px 12px;
  font-size: 14px;
  text-align: left;
  transition: background 0.15s, border-color 0.15s;
}

.nav-item:hover {
  background: var(--bg-panel-hover);
}

.nav-item.active {
  background: var(--brand-soft);
  border-color: var(--brand);
  font-weight: 600;
}

.nav-icon {
  font-size: 16px;
}

.nav-name {
  flex: 1;
}

.nav-count {
  font-size: 12px;
  color: var(--text-secondary);
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  padding: 0 8px;
  min-width: 24px;
  text-align: center;
}

.nav-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--brand);
}

.sidebar-foot {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 12px;
}

.foot-label {
  font-size: 11px;
  color: var(--text-secondary);
}

.foot-path {
  font-size: 11px;
  color: var(--text-secondary);
  word-break: break-all;
}
</style>
