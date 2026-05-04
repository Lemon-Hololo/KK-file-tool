/**
 * Pixiv 标签整理的命令封装。对应后端 `commands::pixiv_tag`。
 */

import type {
  PixivImageRow,
  PixivTagFetchResult,
  PixivTagPartialPayload
} from "../types/pixivTag";
import { invokeCmd, onEvent } from "./tauri";

/**
 * 同步扫描任务输入目录，返回所有可识别 PID 的图片候选。
 *
 * 这是一个轻量的纯本地操作（不拉网络）。前端拿到后立刻渲染 pending 行，
 * 再调 [`startPixivTagScan`] 启动长任务异步拉 tag。
 */
export function scanPixivImageCandidates(paths: string[]) {
  return invokeCmd<PixivImageRow[]>("scan_pixiv_image_candidates", { paths });
}

/** 启动 Pixiv tag 拉取长任务；入参是 PID 列表，返回 `task_id`。 */
export function startPixivTagScan(pids: string[], taskId?: string | null) {
  return invokeCmd<string>("start_pixiv_tag_scan_task", {
    pids,
    taskId: taskId || null
  });
}

/** 单条 PID 同步重试拉取（失败时抛出错误字符串）。 */
export function fetchPixivTagSingle(pid: string) {
  return invokeCmd<PixivTagFetchResult>("fetch_pixiv_tag_single", { pid });
}

/**
 * 把图片移动到 `<outputDir>/<sanitizedTag>/<basename>`。返回新的绝对路径。
 *
 * 后端会自动 mkdir、解决文件名冲突、跨卷 copy+delete 兜底。
 */
export function moveImageByTag(absPath: string, outputDir: string, tag: string) {
  return invokeCmd<string>("move_image_by_tag_command", {
    absPath,
    outputDir,
    tag
  });
}

/** 订阅 Pixiv tag 拉取增量事件。 */
export function onPixivTagPartial(cb: (payload: PixivTagPartialPayload) => void) {
  return onEvent<PixivTagPartialPayload>("pixiv_tag_partial", cb);
}
