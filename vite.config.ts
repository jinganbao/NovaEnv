import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 开发时注入的 host（用于移动端/局域网调试，桌面端通常为空）
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue()],

  // 防止 Vite 清空终端，保留 Rust 编译输出可见
  clearScreen: false,

  server: {
    // Tauri 约定端口，strictPort 保证与 tauri.conf.json 中 devUrl 一致
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 忽略 Rust 后端目录，避免前端热更新与 cargo 编译互相干扰
      ignored: ["**/src-tauri/**"],
    },
  },
});
