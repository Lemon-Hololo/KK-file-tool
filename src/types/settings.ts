export interface AppSettings {
  keepPolicy: "newest" | "oldest";
  moveTargetPath?: string | null;
  saveRecordEnabled: boolean;
  useLastRecordEnabled: boolean;
  includeCurrentFolderDuplicates: boolean;
  themeMode: "light" | "dark" | "system";
  /** 多线程处理核心数，0 = 自动（全部可用核心） */
  threadCount: number;

  // ---- 性能 ----
  /** 日志保留上限（条）；运行时按此裁剪 `runtime.logs` */
  logMaxLength: number;
  /** IO 并发倍率：实际 IO 并发 = `有效线程数 × 本倍率` */
  ioConcurrencyMultiplier: number;
  /** 虚拟表进入"极限模式"的行数阈值 */
  extremeRowThreshold: number;

  // ---- 预览 ----
  /** 文本预览最大读取字节数，以 KB 为单位 */
  textPreviewMaxKb: number;
  /** 压缩包预览枚举的最大条目数 */
  zipPreviewMaxEntries: number;

  // ---- 工具默认值 ----
  /** Mod 扫描关键字的默认值 */
  modScanDefaultKeyword: string;
  /** 后缀修改的默认目标（不带点） */
  suffixDefaultTarget: string;
}

export interface DbPathInfo {
  currentPath: string;
  defaultPath: string;
  customPath: string | null;
}
