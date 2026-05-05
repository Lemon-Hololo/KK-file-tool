/**
 * 路径相关的小工具。
 *
 * 与后端 [`utils/path.rs`](../../src-tauri/src/utils/path.rs) 不同：
 * 这里仅做"展示侧"处理（去重、去 Windows 长路径前缀、取 basename），
 * 不涉及 `\\?\` 加前缀或扩展长度路径转换 —— 那些都在后端做。
 */

/** 去重 + 去除空白条目；用于路径输入框的快速清洗。 */
export function uniquePaths(paths: string[]): string[] {
  return Array.from(new Set(paths.map((p) => p.trim()).filter(Boolean)));
}

/** 去掉 Windows 扩展长度前缀 `\\?\` / `\\?\UNC\`；用户友好显示路径。 */
export function stripWindowsExtendedPrefix(p: string): string {
  if (!p) return p;
  if (p.startsWith("\\\\?\\UNC\\")) return "\\\\" + p.slice(8);
  if (p.startsWith("\\\\?\\")) return p.slice(4);
  return p;
}

/** 取 basename：兼容 `/` 和 `\\` 分隔；先去扩展前缀再切，避免误把 `\\?\C:` 当成路径段。 */
export function baseName(p: string): string {
  if (!p) return p;
  const stripped = stripWindowsExtendedPrefix(p);
  const parts = stripped.split(/[\\/]/);
  return parts[parts.length - 1] || stripped;
}
