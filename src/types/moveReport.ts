import type { DuplicateGroup } from "./task";

export interface MoveSummary {
  targetDir: string;
  totalSelected: number;
  totalSize: number;
}

export interface MoveSuccessItem {
  srcPath: string;
  dstPath: string;
  size: number;
}

export interface MoveFailureItem {
  srcPath: string;
  errorCode: string;
  errorMessage: string;
}

export interface MoveReport {
  reportId: string;
  taskId: string;
  createdAt: number;
  targetDir: string;
  totalSelected: number;
  totalSuccess: number;
  totalFailed: number;
  releasedBytes: number;
  successItems: MoveSuccessItem[];
  failedItems: MoveFailureItem[];
}

export interface MoveActionResponse {
  report: MoveReport;
  updatedGroups: DuplicateGroup[];
}