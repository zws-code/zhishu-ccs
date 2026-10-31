import { useQuery } from "@tanstack/react-query";
import { platformUsageApi } from "@/zhishu/api/platformUsage";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";

export type {
  PlatformSubscriptionUsage,
  PlatformUsageWindow,
} from "@/zhishu/api/platformUsage";

const REFETCH_INTERVAL = 5 * 60 * 1000;

export const platformUsageKeys = {
  all: ["platformUsage"] as const,
  subscriptions: (origin: string, userId: number) =>
    [...platformUsageKeys.all, "subscriptions", origin, userId] as const,
};

/**
 * 按平台地址和用户 ID 隔离订阅额度缓存，所有托管供应商卡片共享一次查询。
 * 五分钟刷新一次，后台不定时刷新；认证请求由 Rust 完成，JWT 不进入前端。
 */
export function usePlatformSubscriptionUsage() {
  const auth = usePlatformAuth();
  const account = auth?.account ?? null;
  return useQuery({
    queryKey: platformUsageKeys.subscriptions(
      account?.baseUrl ?? "",
      account?.user.id ?? 0,
    ),
    queryFn: async ({ signal }) => {
      const summary = await platformUsageApi.subscriptions();
      if (
        signal.aborted ||
        summary.baseUrl !== account?.baseUrl ||
        summary.userId !== account?.user.id
      ) {
        throw { code: "UNAUTHORIZED", message: "登录状态已改变，请重新查询" };
      }
      return summary;
    },
    enabled: Boolean(account),
    staleTime: REFETCH_INTERVAL,
    refetchInterval: REFETCH_INTERVAL,
    refetchIntervalInBackground: false,
    retry: 1,
  });
}
