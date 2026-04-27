export const GROUP_PAGE_SIZES = [10, 20, 50, 100];
export const DEFAULT_GROUP_PAGE_SIZE = 20;

export const DEFAULT_VISIBLE_GROUPS_PER_PAGE = 12;

// 普通模式
export const DEFAULT_GROUP_FILE_RENDER_LIMIT = 300;
export const GROUP_FILE_RENDER_STEP = 300;

// 极限数据阈值
export const EXTREME_GROUP_THRESHOLD = 2000;
export const EXTREME_FILE_THRESHOLD = 5000;

/**
 * 虚拟表/面板启用极限模式的行数阈值 —— 编译期兜底值。
 *
 * 真实生效值由 `useConfigStore().settings.extremeRowThreshold` 提供；
 * 面板在 config store 还未初始化时退回这里。
 */
export const DEFAULT_EXTREME_ROW_THRESHOLD = 20000;

// 极限模式降级参数
export const EXTREME_GROUP_FILE_RENDER_LIMIT = 100;
export const EXTREME_GROUP_FILE_RENDER_STEP = 100;
export const EXTREME_OVERSCAN = 4;
export const NORMAL_OVERSCAN = 10;
