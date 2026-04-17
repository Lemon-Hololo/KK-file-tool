export interface AppSettings {
  keepPolicy: "newest" | "oldest";
  moveTargetPath?: string | null;
  saveRecordEnabled: boolean;
  useLastRecordEnabled: boolean;
  includeCurrentFolderDuplicates: boolean;
  themeMode: "light" | "dark" | "system";
  /** 多线程处理核心数，0 = 自动（全部可用核心） */
  threadCount: number;
}

export interface DbPathInfo {
  currentPath: string;
  defaultPath: string;
  customPath: string | null;
}
