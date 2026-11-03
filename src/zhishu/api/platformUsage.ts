import { invoke } from "@tauri-apps/api/core";

/** Nested fields mirror the dedicated client protocol; no plaintext credentials. */
export interface PlatformQuota {
  available: boolean;
  reasons: string[];
}
export interface PlatformUsageWindow {
  limit_usd: number | null;
  used_usd: number;
  remaining_usd: number | null;
  resets_at: string | null;
}
export interface PlatformSubscriptionUsage {
  id: number;
  group_id: number;
  status: string;
  starts_at: string;
  expires_at: string;
  active: boolean;
  daily: PlatformUsageWindow;
  weekly: PlatformUsageWindow;
  monthly: PlatformUsageWindow;
}
export interface PlatformCredentialMetadata {
  id: number;
  name: string;
  status: string;
  client_managed: boolean;
  expires_at: string | null;
  quota_usd: number;
  used_usd: number;
}
export interface PlatformUsageGroup {
  id: number;
  name: string;
  platform: string;
  status: string;
  subscription_type: string;
  quota: PlatformQuota;
  subscriptions: PlatformSubscriptionUsage[];
  credential: PlatformCredentialMetadata | null;
}
export interface PlatformUsageSummary {
  baseUrl: string;
  userId: number;
  syncedAt: number;
  balance: number;
  groups: PlatformUsageGroup[];
}
export const platformUsageApi = {
  subscriptions: () =>
    invoke<PlatformUsageSummary>("platform_get_subscription_usage"),
};
