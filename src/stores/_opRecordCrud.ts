/**
 * 共享的"可撤回操作记录"CRUD action 工厂。
 *
 * suffix / emptyDirs / modTools 三个 store 都有完全相同形状的 list / detail /
 * delete / rename / checkRollback / rollback / removeBatch action，仅命令名和
 * 业务字段不同。后端通过 `OpRecordTables` 描述符统一了 SQL，前端这里通过本
 * 工厂统一成同一份 action 代码 —— 后续新增同类业务只需写自己的 service 函数
 * 然后 spread 一次工厂结果即可。
 *
 * # 用法（Pinia options-style）
 * ```ts
 * actions: {
 *   ...createOpRecordCrudActions<SuffixRecordSummary, SuffixRecordDetail>({
 *     list: listSuffixChangeRecords,
 *     loadDetail: getSuffixChangeRecordDetail,
 *     remove: deleteSuffixChangeRecord,
 *     checkRollback: checkSuffixRollback,
 *     rollback: rollbackSuffixChange,
 *   }),
 *   async preview(...) { ... },   // 业务专属 action
 *   async apply(...) { ... },
 * }
 * ```
 *
 * # 重命名
 * 哈希记录走独立 store 不在此工厂范围内。Mod 操作记录提供 rename，
 * suffix / emptyDirs 不提供 —— 通过两个工厂函数分别处理：
 * - `createOpRecordCrudActions(deps)`：不含 rename
 * - `createOpRecordCrudActionsWithRename(deps)`：含 rename
 *
 * 拆分两个函数是为了让 TypeScript 在 spread 后给 store 推断出确定的 action
 * 形状（不含联合类型），调用方点 `store.rename` 时能直接命中类型。
 *
 * # 约定
 * - state 必须有 `records: Summary[]` 和 `currentDetail: Detail | null` 字段；
 *   action 内部用 `this.records` / `this.currentDetail` 读写。
 */

import type { OpRecordSummary, OpRollbackCheck, OpRollbackResponse } from "../types/opRecord";

type SummaryWithRecordId = OpRecordSummary<Record<string, unknown>>;
type DetailWithSummary<S> = { summary: S; items: unknown[] };

/** 共享 CRUD 依赖接口（不含 rename）。 */
export interface OpRecordCrudDeps<
  Summary extends SummaryWithRecordId,
  Detail extends DetailWithSummary<Summary>
> {
  list: (kind?: string | null) => Promise<Summary[]>;
  loadDetail: (recordId: string) => Promise<Detail>;
  remove: (recordId: string) => Promise<void>;
  checkRollback: (recordId: string, itemIds?: number[] | null) => Promise<OpRollbackCheck>;
  rollback: (
    recordId: string,
    itemIds?: number[] | null,
    forceIgnoreMissing?: boolean
  ) => Promise<OpRollbackResponse>;
}

/** 含 rename 的依赖接口。 */
export interface OpRecordCrudDepsWithRename<
  Summary extends SummaryWithRecordId,
  Detail extends DetailWithSummary<Summary>
> extends OpRecordCrudDeps<Summary, Detail> {
  rename: (recordId: string, newName: string) => Promise<void>;
}

function buildBaseActions<
  Summary extends SummaryWithRecordId,
  Detail extends DetailWithSummary<Summary>
>(deps: OpRecordCrudDeps<Summary, Detail>) {
  return {
    /** 刷新记录列表；Mod store 可传 kind 过滤。 */
    async refreshRecords(this: { records: Summary[] }, kind?: string | null) {
      this.records = await deps.list(kind ?? null);
    },

    /** 加载并缓存记录详情，供详情抽屉读取。 */
    async loadDetail(this: { currentDetail: Detail | null }, recordId: string) {
      this.currentDetail = await deps.loadDetail(recordId);
      return this.currentDetail;
    },

    /** 撤回前检查；薄透传，不动 state。 */
    checkRollback(_recordId: string, _itemIds?: number[] | null) {
      return deps.checkRollback(_recordId, _itemIds);
    },

    /** 执行撤回；薄透传。 */
    rollback(_recordId: string, _itemIds?: number[] | null, _forceIgnoreMissing = false) {
      return deps.rollback(_recordId, _itemIds, _forceIgnoreMissing);
    },

    /** 删除一条记录后刷新列表；如果删的就是当前抽屉记录，顺手清掉详情。 */
    async remove(
      this: {
        records: Summary[];
        currentDetail: Detail | null;
        refreshRecords: (kind?: string | null) => Promise<void>;
      },
      recordId: string
    ) {
      await deps.remove(recordId);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail = null;
      }
    },

    /** 批量删除；串行调用避免数据库写入冲突，最后整体刷新一次列表。 */
    async removeBatch(
      this: {
        records: Summary[];
        currentDetail: Detail | null;
        refreshRecords: (kind?: string | null) => Promise<void>;
      },
      recordIds: string[]
    ) {
      for (const id of recordIds) {
        await deps.remove(id);
      }
      await this.refreshRecords();
      if (this.currentDetail && recordIds.includes(this.currentDetail.summary.recordId)) {
        this.currentDetail = null;
      }
    }
  };
}

/** 创建不含 rename 的 CRUD actions。供 suffix / emptyDirs 使用。 */
export function createOpRecordCrudActions<
  Summary extends SummaryWithRecordId,
  Detail extends DetailWithSummary<Summary>
>(deps: OpRecordCrudDeps<Summary, Detail>) {
  return buildBaseActions(deps);
}

/** 创建含 rename 的 CRUD actions。供 modTools 使用。 */
export function createOpRecordCrudActionsWithRename<
  Summary extends SummaryWithRecordId,
  Detail extends DetailWithSummary<Summary>
>(deps: OpRecordCrudDepsWithRename<Summary, Detail>) {
  const base = buildBaseActions(deps);
  return {
    ...base,
    /** 重命名记录；命中当前抽屉时同步更新摘要里的 recordName。 */
    async rename(
      this: {
        records: Summary[];
        currentDetail: Detail | null;
        refreshRecords: (kind?: string | null) => Promise<void>;
      },
      recordId: string,
      newName: string
    ) {
      await deps.rename(recordId, newName);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail.summary.recordName = newName;
      }
    }
  };
}
