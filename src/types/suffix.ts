export interface SuffixPreviewItem {
  oldPath: string;
  newPath: string;
  willRenameConflict: boolean;
}

export interface SuffixApplyItem {
  itemId: number;
  oldPath: string;
  newPath: string;
  status: "success" | "failed";
  message?: string;
}

export interface SuffixApplyResponse {
  recordId: string;
  recordName: string;
  total: number;
  success: number;
  failed: number;
  items: SuffixApplyItem[];
}

export interface SuffixRecordSummary {
  recordId: string;
  recordName: string;
  targetSuffix: string;
  createdAt: number;
  totalItems: number;
  successItems: number;
  rollbackStatus: string;
}

export interface SuffixRecordItem {
  itemId: number;
  oldPath: string;
  newPath: string;
  applySuccess: boolean;
  applyError?: string;
  rollbackSuccess?: boolean;
  rollbackError?: string;
}

export interface SuffixRecordDetail {
  summary: SuffixRecordSummary;
  items: SuffixRecordItem[];
}

export interface SuffixRollbackCheck {
  totalSelected: number;
  existingCount: number;
  missingPaths: string[];
}

export interface SuffixRollbackResponse {
  recordId: string;
  totalSelected: number;
  success: number;
  failed: number;
  skippedMissing: number;
  items: SuffixApplyItem[];
}
