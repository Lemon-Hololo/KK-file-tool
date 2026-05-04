/**
 * Pixiv 标签整理的前端类型。
 *
 * 与后端 `models::pixiv_tag` 一一对应，serde camelCase。
 */

/** 扫描候选行：从任务输入识别出的、文件名含合法 PID 的图片。 */
export interface PixivImageRow {
  /** 用户友好路径（已去掉 Windows `\\?\` 前缀）。 */
  absPath: string;
  /** 仅显示用的文件名。 */
  fileName: string;
  /** 文件名中的 8~9 位 PID。 */
  pid: string;
}

/**
 * 单个 PID 的拉取结果。`tags` 与 `error` 互斥。
 *
 * `translations` 仅在成功时携带，是 `tag → translation.en` 的映射；没有英文译名的
 * tag 不会出现在该 Map 里（前端按 missing 时回落原 tag 即可）。
 */
export interface PixivTagPartialItem {
  pid: string;
  tags?: string[] | null;
  translations?: Record<string, string> | null;
  error?: string | null;
}

/** `pixiv_tag_partial` 事件载荷。 */
export interface PixivTagPartialPayload {
  taskId: string;
  items: PixivTagPartialItem[];
  /** `true` 表示扫描终态（成功 / 失败 / 取消），UI 应解锁 running 状态。 */
  done: boolean;
}

/** 单条 PID 重试拉取的返回值。 */
export interface PixivTagFetchResult {
  tags: string[];
  /** `tag → en` 译名映射；没译名的 tag 不在里面。 */
  translations: Record<string, string>;
}

/** 行的拉取状态。 */
export type PixivRowStatus = "pending" | "ok" | "error";

/** 表中一行的完整状态：候选信息 + 拉取结果 + UI 状态。 */
export interface PixivImageState {
  absPath: string;
  fileName: string;
  pid: string;
  status: PixivRowStatus;
  tags: string[];
  /**
   * 该行的 `tag → en` 译名映射；只装"有译名的 tag"。开启翻译开关时面板用这张表
   * 决定 chip 显示文本和点击落盘目录名；没在表里的 tag 沿用原 tag。
   */
  translations: Record<string, string>;
  error: string | null;
  /**
   * 该行最近一次被移动到的 tag 名（即当前文件所在的 `<output>/<tag>/` 子目录）。
   * `null` 表示尚未移动过。再次点同一个 tag 时按钮会置灰，避免无意义的二次移动。
   *
   * 这里存的就是"实际落盘用的字符串"——开启翻译时是 en 译名，关闭时是原 tag。
   * 用户在移动后翻转开关，`movedTag` 与当前显示文本对不上是预期行为
   * （文件确实在那个目录里），highlight 不会触发但用户能直接看到 movedTag 列里的目录名。
   */
  movedTag: string | null;
}
