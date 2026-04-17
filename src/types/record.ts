export interface HashIndexEntry {
  hash: string;
  filePath: string;
  fileSize: number;
  mtime: number;
  status: string;
}

export interface HashIndexRecord {
  recordId: string;
  recordName: string;
  createdAt: number;
  sourcePaths: string[];
  entries: HashIndexEntry[];
}

export interface HashIndexRecordSummary {
  recordId: string;
  recordName: string;
  createdAt: number;
  sourcePaths: string[];
  entryCount: number;
}