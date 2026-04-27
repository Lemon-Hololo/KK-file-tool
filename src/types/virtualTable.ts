export interface VirtualColumn {
  key: string;
  label: string;
  width?: number;
  minWidth?: number;
  ellipsis?: boolean;
  slotName?: string;
  formatter?: (row: any, value: any, index: number) => string;
  fixed?: "left" | "right";
  resizable?: boolean;
}

export interface PaginationConfig {
  mode?: "client" | "server";
  page?: number;
  pageSize?: number;
  total?: number;
  pageSizes?: number[];
  show?: boolean;
}

export interface RenderColumn {
  key: string;
  label: string;
  width: number;
  ellipsis: boolean;
  slotName?: string;
  formatter?: (row: any, value: any, index: number) => string;
  fixed?: "left" | "right";
  resizable: boolean;
}

/**
 * 用户对单列的自定义配置。
 *
 * 不含 selection 列（`__select__`）—— 选择列永远是第一列、永远左固定，
 * 不纳入自定义范畴。`fixed` 仅表示左固定；VirtualTable 对用户列只开放左固定。
 *
 * `order` 是该列在"非 selection 列"内的位置（0-based，紧跟 selection 之后渲染）。
 */
export interface VirtualColumnState {
  key: string;
  visible: boolean;
  fixed: boolean;
  order: number;
}
