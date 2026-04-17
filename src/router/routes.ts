import type { RouteRecordRaw } from "vue-router";
import TaskPage from "../views/TaskPage.vue";
import SettingsPage from "../views/SettingsPage.vue";
import RecordManagePage from "../views/RecordManagePage.vue";

export const routes: RouteRecordRaw[] = [
  { path: "/", component: TaskPage },
  { path: "/settings", component: SettingsPage },
  { path: "/records", component: RecordManagePage }
];