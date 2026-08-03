import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";

// 全局禁用右键菜单（浏览器默认 + WebView 系统菜单均不出现）
window.addEventListener("contextmenu", (e) => e.preventDefault());

createApp(App).mount("#app");
