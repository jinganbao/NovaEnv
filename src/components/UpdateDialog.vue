<script setup lang="ts">
import { onMounted } from "vue";
import type { useAppUpdate } from "../composables/useAppUpdate";

const props = defineProps<{
  update: ReturnType<typeof useAppUpdate>;
  onClose: () => void;
}>();

onMounted(() => {
  // 弹窗打开时若尚未检查过，立即触发
  if (!props.update.updateInfo.value && !props.update.checkingUpdate.value) {
    props.update.checkForUpdates();
  }
});
</script>

<template>
  <Teleport to="body">
    <div class="upd-mask" @click.self="onClose">
      <div class="upd-card" role="dialog" aria-label="检查更新">
        <!-- 检查中：滚动条 -->
        <template v-if="update.checkingUpdate.value">
          <div class="upd-icon">🔍</div>
          <p class="upd-title">正在检查更新…</p>
          <div class="upd-bar">
            <div class="upd-fill indeterminate"></div>
          </div>
        </template>

        <!-- 检查失败 -->
        <template v-else-if="update.updateError.value">
          <div class="upd-icon">⚠️</div>
          <p class="upd-title">检查更新失败</p>
          <p class="upd-desc">{{ update.updateError.value }}</p>
          <div class="upd-actions">
            <button class="btn btn-primary" @click="update.checkForUpdates()">重试</button>
            <button class="btn" @click="onClose">关闭</button>
          </div>
        </template>

        <!-- 已是最新 -->
        <template v-else-if="update.updateInfo.value && !update.updateInfo.value.hasUpdate">
          <div class="upd-icon ok">✓</div>
          <p class="upd-title">已是最新版本</p>
          <p class="upd-desc">当前版本 v{{ update.currentVersion.value || "—" }}</p>
          <div class="upd-actions">
            <button class="btn btn-primary" @click="onClose">好的</button>
          </div>
        </template>

        <!-- 有可用更新 -->
        <template v-else-if="update.updateInfo.value?.hasUpdate">
          <!-- 升级中：进度条 + 取消 -->
          <template v-if="update.installingUpdate.value">
            <div class="upd-icon">⬇️</div>
            <p class="upd-title">正在升级到 v{{ update.updateInfo.value.version }}</p>
            <div class="upd-bar">
              <div class="upd-fill" :style="{ width: update.updateProgressPercentage.value + '%' }"></div>
            </div>
            <p class="upd-desc">{{ update.updateProgressLabel.value }}</p>
            <div class="upd-actions">
              <button class="btn" :disabled="update.cancellingUpdate.value" @click="update.cancelUpdateDownload()">
                {{ update.cancellingUpdate.value ? "取消中…" : "取消升级" }}
              </button>
            </div>
          </template>

          <!-- 待确认升级 -->
          <template v-else>
            <div class="upd-icon">🎉</div>
            <p class="upd-title">发现新版本</p>
            <p class="upd-desc">
              当前 v{{ update.currentVersion.value || "—" }} →
              <strong class="upd-new">v{{ update.updateInfo.value.version }}</strong>
            </p>
            <p v-if="update.updateInfo.value.date" class="upd-meta">发布日期 {{ update.updateInfo.value.date }}</p>
            <div class="upd-actions">
              <button class="btn btn-primary" @click="update.handleUpdateDownload()">立即升级</button>
              <button class="btn" @click="onClose">稍后</button>
            </div>
          </template>
        </template>

        <!-- 兜底 -->
        <template v-else>
          <p class="upd-title">…</p>
        </template>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.upd-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 110;
  backdrop-filter: blur(2px);
}

.upd-card {
  width: min(360px, 92vw);
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  box-shadow: 0 20px 50px var(--shadow-strong);
  padding: 26px 30px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.upd-icon {
  width: 56px;
  height: 56px;
  border-radius: 14px;
  background: var(--brand-soft);
  display: grid;
  place-items: center;
  font-size: 26px;
  margin-bottom: 8px;
}

.upd-icon.ok {
  background: var(--success-soft);
  color: var(--success);
  font-weight: 700;
}

.upd-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
}

.upd-desc {
  color: var(--text-secondary);
  font-size: 13px;
  text-align: center;
  line-height: 1.6;
  margin: 4px 0 0;
  word-break: break-all;
}

.upd-new {
  color: var(--brand);
}

.upd-meta {
  color: var(--text-muted);
  font-size: 12px;
  margin: 2px 0 0;
}

/* 进度条 */
.upd-bar {
  width: 100%;
  height: 8px;
  background: var(--bg-input);
  border-radius: 99px;
  overflow: hidden;
  margin: 14px 0 4px;
}

.upd-fill {
  height: 100%;
  background: var(--brand);
  border-radius: 99px;
  transition: width 0.2s ease;
}

/* 检查中滚动条（不确定进度） */
.upd-fill.indeterminate {
  width: 35%;
  animation: upd-slide 1.2s ease-in-out infinite;
}

@keyframes upd-slide {
  0% {
    transform: translateX(-110%);
  }
  100% {
    transform: translateX(320%);
  }
}

.upd-actions {
  display: flex;
  justify-content: center;
  gap: 10px;
  margin-top: 16px;
  width: 100%;
}

.btn {
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel-hover);
  color: var(--text-primary);
  border-radius: 8px;
  padding: 8px 18px;
  font-size: 13px;
  cursor: pointer;
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.btn-primary {
  background: var(--brand);
  border-color: var(--brand);
  color: #08130d;
  font-weight: 600;
}

.btn-primary:hover {
  background: var(--brand-hover);
}
</style>
