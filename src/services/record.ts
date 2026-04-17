import type { HashIndexRecord, HashIndexRecordSummary } from "../types/record";
import { invokeCmd } from "./tauri";

export function listHashRecords() {
  return invokeCmd<HashIndexRecordSummary[]>("list_hash_records");
}

export function loadHashRecord(recordId: string) {
  return invokeCmd<HashIndexRecord>("load_hash_record", { recordId });
}

export function renameHashRecord(recordId: string, newName: string) {
  return invokeCmd<void>("rename_hash_record", { recordId, newName });
}

export function deleteHashRecord(recordId: string) {
  return invokeCmd<void>("delete_hash_record", { recordId });
}