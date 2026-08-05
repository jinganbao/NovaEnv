<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { openExternal } from "../api";

const open = ref(false);
const version = ref("");
let unlisten: (() => void) | undefined;

onMounted(async () => {
  unlisten = await listen("novaenv-about", () => {
    open.value = true;
    getVersion().then((v) => (version.value = v)).catch(() => {});
  });
});
onUnmounted(() => unlisten?.());

function goRepo() {
  openExternal("https://github.com/jinganbao/NovaEnv").catch(() => {});
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="about-mask" @click.self="open = false">
      <div class="about-card" role="dialog" aria-label="关于 NovaEnv">
        <div class="about-icon">🛠️</div>
        <h2 class="about-name">NovaEnv</h2>
        <p class="about-ver">v{{ version || "1.0.0" }}</p>
        <p class="about-desc">本地开发环境管理器 · Java / Node.js / Go / Maven / Python / Rust / Redis / MySQL</p>
        <div class="about-rows">
          <div class="about-row"><span>作者</span><span>NovaHub</span></div>
          <div class="about-row"><span>许可证</span><span>MIT</span></div>
          <div class="about-row">
            <span>项目主页</span>
            <button class="link-btn" @click="goRepo">github.com/jinganbao/NovaEnv</button>
          </div>
        </div>
        <p class="about-copy">© 2026 NovaHub · MIT License</p>
        <button class="btn btn-primary about-close" @click="open = false">关闭</button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.about-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 100;
  backdrop-filter: blur(2px);
}

.about-card {
  width: min(380px, 92vw);
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  box-shadow: 0 20px 50px var(--shadow-strong);
  padding: 26px 30px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.about-icon {
  width: 64px;
  height: 64px;
  border-radius: 14px;
  background: var(--brand-soft);
  display: grid;
  place-items: center;
  font-size: 30px;
  margin-bottom: 10px;
}

.about-name {
  font-size: 19px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
}

.about-ver {
  color: var(--brand);
  font-size: 13px;
  font-weight: 600;
  margin: 0;
}

.about-desc {
  color: var(--text-secondary);
  font-size: 12.5px;
  text-align: center;
  line-height: 1.6;
  margin: 10px 0 4px;
}

.about-rows {
  width: 100%;
  margin-top: 10px;
  border-top: 1px solid var(--border-subtle);
  padding-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.about-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
  padding: 5px 0;
}

.about-row span:first-child {
  color: var(--text-muted);
}

.about-row span:last-child {
  color: var(--text-primary);
}

.link-btn {
  background: none;
  border: none;
  color: var(--brand);
  cursor: pointer;
  font-size: 13px;
  padding: 0;
  text-decoration: underline;
  text-underline-offset: 2px;
}

.about-copy {
  color: var(--text-muted);
  font-size: 11.5px;
  margin: 12px 0 0;
}

.about-btns {
  display: flex;
  gap: 10px;
  margin-top: 16px;
}

.about-close {
  min-width: 110px;
}
</style>
