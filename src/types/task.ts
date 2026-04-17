export interface DedupConfig {
  keepPolicy: "newest" | "oldest";
  moveTargetPath?: string | null;
  autoSelectEnabled: boolean;
  saveRecordEnabled: boolean;
  useLastRecordEnabled: boolean;
  selectedRecordId?: string | null;
  includeCurrentFolderDuplicates: boolean;
  recordName?: string | null;
}

export interface FileEntry {
  absPath: string;
  size: number;
  mtime: number;
  ctime: number; // 创建时间
  hash?: string;
  selectedForMove?: boolean;
  fromHistory?: boolean;
}

export interface DuplicateGroup {
  groupId: string;
  hash: string;
  files: FileEntry[];
}

export interface NormalizePathResult {
  normalizedPaths: string[];
  removedPaths: string[];
  warnings: string[];
}