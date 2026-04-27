export const APP_NAME = "FileFlow Desktop";

/**
 * 日志保留上限（条）的**编译期兜底值**。
 *
 * 真实生效值由 `useConfigStore().settings.logMaxLength` 提供；
 * 运行时 store（`stores/runtime.ts`）读不到 store 时退回这里。
 */
export const DEFAULT_LOG_MAX_LENGTH = 3000;

/** 日志批量刷新间隔（ms），平衡实时性与渲染性能。 */
export const LOG_FLUSH_INTERVAL = 150;
