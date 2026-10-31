import { invoke } from "@tauri-apps/api/core";
import type {
  PlatformQuota,
  PlatformSubscriptionUsage,
  PlatformCredentialMetadata,
} from "./platformUsage";
import type { AppId } from "@/lib/api/types";

/** 本地同步结果：数据库行变化与原生文件变化分别统计，部分成功仍需展示 warnings。 */
export interface PlatformSyncSummary {
  added: number;
  removed: number;
  skipped: number;
  warnings: string[];
  updated?: number;
  nativeFilesChanged?: number;
}

/** 显式删除结果；skipped 为未能安全删除的目标数，warnings 包含原因。 */
export interface PlatformDeleteSummary {
  removed: number;
  skipped: number;
  warnings: string[];
  nativeFilesChanged: number;
  /** 全应用删除时返回；应用级失败的数量不可用，应展示 error。 */
  apps?: PlatformDeleteAppResult[];
}

export interface PlatformDeleteAppResult {
  agent: AppId;
  removed: number;
  skipped: number;
  warnings: string[];
  nativeFilesChanged: number;
  error?: { code: string; message: string };
}

/** 当前账户的目录摘要，不含模型 Key；syncedAt 使用 Unix 毫秒。 */
export interface PlatformCatalog {
  baseUrl: string;
  userId: number;
  syncedAt: number;
  groups: PlatformCatalogGroup[];
  warnings: string[];
}

export interface PlatformCatalogGroup {
  id: number;
  name: string;
  platform: string;
  subscriptionType: string;
  status?: string;
  keyId?: number | null;
  keyName?: string | null;
  keyAvailable?: boolean;
  quota?: PlatformQuota | null;
  subscriptions?: PlatformSubscriptionUsage[];
  credential?: PlatformCredentialMetadata | null;
  failed: boolean;
  error: string | null;
  models: PlatformCatalogModel[];
}

export interface PlatformCatalogModel {
  id: string;
  name: string;
  description?: string | null;
  contextWindow?: number | null;
  maxOutputTokens?: number | null;
  endpoints?: string[];
  supportsTools?: boolean | null;
  supportsStreaming?: boolean | null;
  inputModalities?: string[];
  reasoningLevels?: string[];
  defaultReasoningLevel?: string | null;
}

export const platformModelsApi = {
  // null 删除当前账户在全部已支持应用中的托管供应商。
  deleteProviders: (agent: AppId | null, baseUrl: string, userId: number) =>
    invoke<PlatformDeleteSummary>("platform_delete_providers", {
      agent,
      baseUrl,
      userId,
    }),
  // 页面读取只访问设备快照，不触发平台拉取或工具配置写入。
  catalog: () => invoke<PlatformCatalog | null>("platform_get_catalog"),
  // 单个 Key 仅用于显式查看/复制，不得放入 TanStack Query 缓存。
  catalogKey: (baseUrl: string, userId: number, groupId: number) =>
    invoke<string | null>("platform_get_catalog_key", {
      baseUrl,
      userId,
      groupId,
    }),
  // 第一阶段通过客户端 access 接口更新目录，Key 生命周期由服务端管理；第二阶段仅按本机目录更新工具供应商。
  syncCatalog: () => invoke<PlatformCatalog>("platform_sync_catalog"),
  sync: (agent?: AppId) =>
    invoke<PlatformSyncSummary>("platform_sync_providers", {
      agent: agent ?? null,
    }),
};
