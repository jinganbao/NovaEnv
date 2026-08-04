<script setup lang="ts">
import type { RuntimeKind, RuntimeVersion } from "../types";
import { RUNTIME_META } from "../types";
import RuntimeVersionList from "./RuntimeVersionList.vue";

const props = defineProps<{ kind: RuntimeKind; versions: RuntimeVersion[] }>();
const emit = defineEmits<{
  (e: "activate", version: RuntimeVersion): void;
  (e: "uninstall", version: RuntimeVersion): void;
  (e: "refresh"): void;
}>();

function defaultVersion(): RuntimeVersion | undefined {
  return props.versions.find((v) => v.isDefault);
}
</script>

<template>
  <div class="detail">
    <header class="page-head">
      <span
        class="head-icon"
        :style="{ background: RUNTIME_META[kind].color }"
        >{{ RUNTIME_META[kind].letter }}</span
      >
      <div class="head-text">
        <h2>{{ RUNTIME_META[kind].name }}</h2>
        <p>{{ RUNTIME_META[kind].desc }}</p>
      </div>
      <div class="head-status">
        <span v-if="defaultVersion()" class="badge badge-success">
          <span class="dot"></span>
          当前默认 {{ defaultVersion()!.version }}
        </span>
        <span v-else class="badge badge-warning">未设置默认版本</span>
      </div>
    </header>

    <RuntimeVersionList
      :kind="kind"
      :installed="versions"
      @activate="emit('activate', $event)"
      @uninstall="emit('uninstall', $event)"
      @refresh="emit('refresh')"
    />
  </div>
</template>

<style scoped>
.detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

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
}

.head-text {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

h2 {
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

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}
</style>
