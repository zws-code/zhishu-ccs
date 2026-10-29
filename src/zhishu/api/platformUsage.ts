import { invoke } from "@tauri-apps/api/core";

export interface PlatformUsageWindow {
  limitUsd: number;
  usedUsd: number;
  remainingUsd: number;
  percentage: number;
  resetsAt: string | null;
}

/** 订阅分组的额度快照；过期、暂停和进度缺失必须按状态展示，不能按缺省值推断不限量。 */
export interface PlatformSubscriptionUsage {
  subscriptionId: number;
  groupId: number;
  groupName: string;
  groupPlatform: string;
  expiresAt: string;
  expiresInDays: number;
  unlimited: boolean;
  status?: string;
  daily?: PlatformUsageWindow;
  weekly?: PlatformUsageWindow;
  monthly?: PlatformUsageWindow;
}

export const platformUsageApi = {
  subscriptions: () =>
    invoke<PlatformSubscriptionUsage[]>("platform_get_subscription_usage"),
};
