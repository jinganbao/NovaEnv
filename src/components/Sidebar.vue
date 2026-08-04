<script setup lang="ts">
import type { RuntimeKind, ServiceInfo } from "../types";
import { RUNTIME_META, SERVICE_META } from "../types";

defineProps<{
  kinds: RuntimeKind[];
  selected: RuntimeKind | ServiceInfo["kind"] | "settings";
  counts: Record<RuntimeKind, number>;
  services: ServiceInfo[];
}>();
const emit = defineEmits<{
  (e: "select", kind: RuntimeKind | ServiceInfo["kind"] | "settings"): void;
}>();

/** 运行时/服务品牌色与字母（来自共享元数据） */
function metaOf(kind: RuntimeKind | ServiceInfo["kind"]): { letter: string; color: string } {
  if (kind === "redis" || kind === "mysql") return SERVICE_META[kind];
  return RUNTIME_META[kind];
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
        <span
          class="nav-badge"
          :style="{ background: metaOf(k).color }"
          >{{ metaOf(k).letter }}</span
        >
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
          class="nav-badge"
          :style="{ background: metaOf(s.kind).color }"
          >{{ metaOf(s.kind).letter }}</span
        >
        <span class="nav-name">{{ s.name }}</span>
        <span
          class="svc-state"
          :class="s.installed ? (s.running ? 'on' : 'off') : 'none'"
          :title="
            !s.installed
              ? '未安装'
              : s.running
                ? `运行中（${s.version}）`
                : `已停止（${s.version}）`
          "
        ></span>
        <span class="nav-count">{{ s.installed ? s.version : "未安装" }}</span>
      </button>
      <div v-if="!services.length" class="nav-empty">
        <span class="nav-empty-text">暂无服务组件</span>
      </div>
    </nav>

    <div class="sidebar-foot">
      <button
        class="nav-item"
        :class="{ active: selected === 'settings' }"
        @click="emit('select', 'settings')"
      >
        <span class="nav-badge" :style="{ background: 'linear-gradient(135deg,#94a3b8,#475569)' }">⚙</span>
        <span class="nav-name">设置</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 208px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  border-right: 1px solid var(--border-subtle);
  background: var(--bg-sider);
  padding: var(--space-4) var(--space-3);
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  scrollbar-width: thin;
}

.group-label {
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.08em;
  color: var(--text-muted);
  text-transform: uppercase;
  padding: var(--space-3) var(--space-3) var(--space-2);
}

.group-label:first-child {
  padding-top: var(--space-1);
}

.nav-empty {
  padding: var(--space-2) var(--space-3);
  margin: var(--space-1) var(--space-1) 0;
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-md);
}

.nav-empty-text {
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-secondary);
  border-radius: var(--radius-md);
  padding: 8px 12px;
  font-size: var(--text-md);
  text-align: left;
  transition: background var(--duration) var(--ease), color var(--duration) var(--ease);
}

.nav-item::before {
  content: "";
  position: absolute;
  left: -12px;
  top: 50%;
  transform: translateY(-50%) scaleY(0);
  width: 3px;
  height: 20px;
  border-radius: 0 3px 3px 0;
  background: var(--brand);
  transition: transform var(--duration) var(--ease);
}

.nav-item:hover {
  background: var(--bg-panel);
  color: var(--text-primary);
}

.nav-item.active {
  background: var(--brand-soft);
  color: var(--text-primary);
  font-weight: 600;
}

.nav-item.active::before {
  transform: translateY(-50%) scaleY(1);
}

/* 品牌色字母块 */
.nav-badge {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
}

.nav-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav-count {
  font-size: var(--text-xs);
  color: var(--text-muted);
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-pill);
  padding: 0 8px;
  min-width: 22px;
  text-align: center;
  white-space: nowrap;
}

.nav-item.active .nav-count {
  color: var(--brand);
  border-color: transparent;
  background: rgba(52, 211, 153, 0.1);
}

/* 服务状态点 */
.svc-state {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.svc-state.on {
  background: var(--success);
  box-shadow: 0 0 0 3px var(--success-soft);
}

.svc-state.off {
  background: var(--warning);
}

.svc-state.none {
  background: var(--text-muted);
}

.sidebar-foot {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border-top: 1px solid var(--border-subtle);
  padding-top: var(--space-3);
  margin-top: var(--space-2);
  flex-shrink: 0;
}
</style>
