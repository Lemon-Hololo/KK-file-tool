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
