/** 文件预览的命令封装。对应后端 `commands::preview`。 */

import type { PreviewPayload } from "../types/preview";
import { invokeCmd } from "./tauri";

/** 请求指定路径的预览信息；后端按扩展名路由到 text/image/zip 处理。 */
export function requestPreview(filePath: string) {
  return invokeCmd<PreviewPayload>("request_preview", { filePath });
}
