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

  // ---- Mod 工具回滚 ----
  /**
   * 是否启用 Mod 工具的备份/回滚机制（重复删除 / 不同版本删除 / 移除版本限制）。
   * 关闭后这三类操作不再创建备份，记录的"撤回"按钮置灰。重命名 / 归类不受影响。
   */
  modRollbackEnabled: boolean;
  /** Mod 备份目录；为空时使用 `<exe_dir>/mod-backups`。 */
  modBackupDir?: string | null;

  // ---- Pixiv 标签整理 ----
  /** Pixiv 标签接口的 base URL；最终请求 `<base><pid>`。 */
  pixivTagApiBase: string;
  /** 排除的 tag 列表；这些 tag 不会作为虚拟表的列出现。 */
  pixivExcludedTags: string[];
  /** 本地 tag 翻译表；key 为 Pixiv 原 tag，value 为本地译名。 */
  pixivLocalTagTranslations: Record<string, string>;
  /** 可选 Pixiv Cookie；填了之后能拿到 R-18 / 关注限定等受限 tag。 */
  pixivCookie?: string | null;
  /** 可选 HTTP / HTTPS / SOCKS5 代理 URL；中国大陆访问 Pixiv 一般要配。 */
  pixivProxy?: string | null;
  /**
   * 是否在面板上用 `translation.en` 替代原 tag 显示（同时点击移动也用译名做目录）。
   * 任务面板顶部和配置中心是同一个开关，改一处两处都生效。
   */
  pixivUseTranslation: boolean;
  /**
   * Pixiv 拉取的每分钟最大请求数（限速防黑）；0 视为不限速，UI 限制最小 1。
   *
   * 60 = 每秒 1 条；与并发上限正交——并发只控制"同时在飞的请求数"，本设置控制
   * "任意 60 秒滚动窗口内总请求数"。所有并发 worker 共享同一条 next-slot 队列，
   * 单条重试也会去同一队列排号，所以即便用户狂按重试也不会把瞬时速率打穿。
   */
  pixivRateLimitPerMinute: number;
  /**
   * Pixiv 增量结果在前端的合并刷新间隔（毫秒）。
   *
   * 0 = 即刻：partial 一到达就立刻应用，UI 跟随每条结果跳动；
   * >0 = 节流：partial 进入缓冲区，按本间隔批量 commit。done 终态会立刻 flush。
   * UI 限制 0–10000ms。50K 张图配合 500ms 节流时屏幕节奏明显平稳。
   */
  pixivPartialFlushIntervalMs: number;
}

export interface DbPathInfo {
  currentPath: string;
  defaultPath: string;
  customPath: string | null;
}
