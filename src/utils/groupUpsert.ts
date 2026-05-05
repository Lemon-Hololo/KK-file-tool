/**
 * 按 `groupId` 合并增量分组结果。
 *
 * 三处增量推送都需要"已有列表 + 新一批 partial → 按 groupId 去重 + 排序"：
 * - 去重 task：`task_result_partial`
 * - Mod 重复检查：`mod_duplicate_partial`
 * - Mod 不同版本：`mod_version_partial`
 *
 * 抽到这里避免每个 store 各写一份 Map upsert + sort。
 */

export interface HasGroupId {
  groupId: string;
}

/**
 * 合并 `incoming` 到 `existing`，按 `groupId` 去重（incoming 覆盖 existing），
 * 并按 `groupId` 字典序排序后返回新数组。
 */
export function upsertGroupsByKey<G extends HasGroupId>(existing: G[], incoming: G[]): G[] {
  const map = new Map(existing.map((g) => [g.groupId, g]));
  for (const g of incoming) map.set(g.groupId, g);
  return Array.from(map.values()).sort((a, b) => a.groupId.localeCompare(b.groupId));
}
