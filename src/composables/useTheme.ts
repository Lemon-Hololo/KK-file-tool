import { computed, watch } from "vue";
import { useDark, usePreferredDark, useStorage } from "@vueuse/core";

export type ThemeMode = "light" | "dark" | "system";

/**
 * 主题模式来源（用户选择）：
 * - light: 强制浅色
 * - dark: 强制暗色
 * - system: 跟随系统
 */
const themeMode = useStorage<ThemeMode>("themeMode", "system");

/**
 * 当前系统是否偏好暗色（响应系统变化）
 */
const preferredDark = usePreferredDark();

/**
 * 实际是否启用暗色（应用层）
 */
const isDark = useDark({
  selector: "html",
  attribute: "class",
  valueDark: "dark",
  valueLight: ""
});

/**
 * 计算实际暗色开关值
 */
const effectiveDark = computed(() => {
  if (themeMode.value === "dark") return true;
  if (themeMode.value === "light") return false;
  return preferredDark.value;
});

// 同步到 DOM（html.classList）
watch(
  effectiveDark,
  (val) => {
    isDark.value = val;
  },
  { immediate: true }
);

export function useTheme() {
  const setThemeMode = (mode: ThemeMode) => {
    themeMode.value = mode;
  };

  const initTheme = () => {
    isDark.value = effectiveDark.value;
  };

  return {
    themeMode,
    isDark,
    preferredDark,
    effectiveDark,
    setThemeMode,
    initTheme
  };
}
