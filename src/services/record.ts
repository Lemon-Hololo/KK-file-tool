/** 哈希记录 CRUD 的命令封装。对应后端 `commands::records`。 */

import type { HashIndexRecord, HashIndexRecordSummary } from "../types/record";
import { invokeCmd } from "./tauri";

/** 列出所有哈希记录摘要。 */
export function listHashRecords() {
  return invokeCmd<HashIndexRecordSummary[]>("list_hash_records");
}

/** 读取单条记录的完整详情（含全部 entries）。 */
export function loadHashRecord(recordId: string) {
  return invokeCmd<HashIndexRecord>("load_hash_record", { recordId });
}

/** 重命名记录。 */
export function renameHashRecord(recordId: string, newName: string) {
  return invokeCmd<void>("rename_hash_record", { recordId, newName });
}

/** 删除记录（级联删除 entries）。 */
export function deleteHashRecord(recordId: string) {
  return invokeCmd<void>("delete_hash_record", { recordId });
}
