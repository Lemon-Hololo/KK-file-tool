import { createApp } from "vue";
import { createPinia } from "pinia";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import "./styles/index.css";

import App from "./App.vue";
import router from "./router";
import { useConfigStore } from "./stores/config";
import { useTheme } from "./composables/useTheme";

async function bootstrap() {
  const app = createApp(App);
  const pinia = createPinia();

  app.use(pinia);
  app.use(router);
  app.use(ElementPlus);

  // 先初始化主题，避免首屏闪白
  const { initTheme } = useTheme();
  initTheme();

  // 再加载后端配置并覆盖主题模式
  const configStore = useConfigStore();
  await configStore.loadSettings();

  app.mount("#app");
}

bootstrap();
