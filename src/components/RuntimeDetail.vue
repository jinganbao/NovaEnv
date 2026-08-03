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
    <header class="detail-head">
      <span class="icon">{{ RUNTIME_META[kind].icon }}</span>
      <h2>{{ RUNTIME_META[kind].name }}</h2>
      <span class="desc">{{ RUNTIME_META[kind].desc }}</span>
      <span v-if="defaultVersion()" class="current">
        当前默认：<strong>{{ defaultVersion()!.version }}</strong>
      </span>
      <span v-else class="current muted">未设置默认版本</span>
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
  gap: 18px;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--border-subtle);
}

.icon {
  font-size: 22px;
}

h2 {
  font-size: 18px;
}

.desc {
  color: var(--text-secondary);
  font-size: 13px;
}

.current {
  margin-left: auto;
  color: var(--text-secondary);
  font-size: 13px;
}

.current strong {
  color: var(--success);
}

.current.muted {
  color: var(--text-secondary);
}
</style>
