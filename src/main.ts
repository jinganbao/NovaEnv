import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";

// 全局禁用右键菜单（浏览器默认 + WebView 系统菜单均不出现）
window.addEventListener("contextmenu", (e) => e.preventDefault());

// ---- 全局错误捕获：错误遮罩替代白屏 ----
let fatalShown = false;
function showFatalError(message: string) {
  if (fatalShown) return;
  fatalShown = true;
  const overlay = document.createElement("div");
  overlay.style.cssText =
    "position:fixed;inset:0;z-index:9999;background:var(--bg-app,#111418);" +
    "display:flex;flex-direction:column;align-items:center;justify-content:center;" +
    "gap:16px;padding:32px;text-align:center;";
  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  overlay.innerHTML = `
    <div style="font-size:15px;color:var(--text-primary,#E7ECF3);font-weight:600">NovaEnv 遇到问题</div>
    <div style="font-size:12px;color:var(--text-secondary,#9AA5B5);max-width:560px;word-break:break-all;line-height:1.6">${esc(message)}</div>
    <button id="novaenv-reload" style="border:1px solid var(--border-subtle,#39424E);background:var(--brand,#34D399);color:#fff;border-radius:8px;padding:8px 22px;font-size:13px;cursor:pointer">重新加载</button>`;
  overlay
    .querySelector("#novaenv-reload")!
    .addEventListener("click", () => location.reload());
  document.body.appendChild(overlay);
}

window.addEventListener("error", (e) => showFatalError(e.message || "未知错误"));
window.addEventListener("unhandledrejection", (e) => {
  const reason =
    e.reason instanceof Error ? e.reason.message : String(e.reason ?? "未知错误");
  showFatalError(reason);
});

const app = createApp(App);
app.config.errorHandler = (err, _instance, info) => {
  console.error("[NovaEnv]", info, err);
  showFatalError(err instanceof Error ? err.message : String(err));
};
app.mount("#app");
