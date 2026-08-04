<script setup lang="ts">
import type { RuntimeKind, ServiceInfo } from "../types";
import { RUNTIME_META } from "../types";

defineProps<{
  kinds: RuntimeKind[];
  selected: RuntimeKind | ServiceInfo["kind"] | "settings";
  counts: Record<RuntimeKind, number>;
  services: ServiceInfo[];
}>();
const emit = defineEmits<{
  (e: "select", kind: RuntimeKind | ServiceInfo["kind"] | "settings"): void;
}>();

function isKind(value: RuntimeKind | ServiceInfo["kind"] | "settings"): value is RuntimeKind {
  return value !== "settings" && value !== "redis" && value !== "mysql";
}
</script>

<template>
  <aside class="sidebar">
    <nav class="nav">
      <div class="group-label">语言运行时</div>
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

      <div class="group-label">服务类组件</div>
      <button
        v-for="s in services"
        :key="s.kind"
        class="nav-item"
        :class="{ active: selected === s.kind }"
        @click="emit('select', s.kind)"
      >
        <span
          class="svc-dot"
          :class="s.installed ? (s.running ? 'on' : 'off') : 'none'"
        ></span>
        <span class="nav-name">{{ s.name }}</span>
        <span class="nav-count">{{ s.installed ? s.version : "未安装" }}</span>
      </button>
      <div v-if="!services.length" class="nav-empty">
        <span class="nav-empty-text">暂无服务</span>
      </div>
    </nav>

    <div class="sidebar-foot">
      <button
        class="settings-btn"
        :class="{ active: selected === 'settings' }"
        @click="emit('select', 'settings')"
      >
        <span class="nav-icon">⚙️</span>
        <span class="nav-name">设置</span>
        <span v-if="!isKind(selected)" class="nav-dot"></span>
      </button>
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
  gap: 4px;
}

.group-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  color: var(--text-muted);
  text-transform: uppercase;
  padding: 10px 12px 4px;
}

.group-label:first-child {
  padding-top: 2px;
}

.nav-empty {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 12px 10px;
  margin-top: 2px;
  border: 1px dashed var(--border-subtle);
  border-radius: 10px;
}

.nav-empty-text {
  font-size: 12px;
  color: var(--text-secondary);
}

.nav-empty-sub {
  font-size: 11px;
  color: var(--text-muted);
}

.svc-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.svc-dot.on {
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
}

.svc-dot.off {
  background: var(--warning);
}

.svc-dot.none {
  background: var(--text-muted);
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
  gap: 8px;
  border-top: 1px solid var(--border-subtle);
  padding-top: 12px;
}

.settings-btn {
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

.settings-btn:hover {
  background: var(--bg-panel-hover);
}

.settings-btn.active {
  background: var(--brand-soft);
  border-color: var(--brand);
  font-weight: 600;
}
</style>
