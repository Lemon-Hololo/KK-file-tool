import type { PreviewPayload } from "../types/preview";
import { invokeCmd } from "./tauri";

export function requestPreview(filePath: string) {
  return invokeCmd<PreviewPayload>("request_preview", { filePath });
}