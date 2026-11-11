import type { Provider } from "@/types";
import type { PlatformAccount } from "@/zhishu/api/platformAuth";

/**
 * 当前登录账号的平台托管判定：`provider.meta.platformManaged` 的来源与用户
 * 都匹配本账号。简易模式的开关状态、副标题和供应商过滤统一用这里；
 * 既有调用点的内联写法保持原样，不做批量替换。
 */
export function isAccountManaged(
  provider: Provider | undefined | null,
  account: PlatformAccount | null | undefined,
): boolean {
  const managed = provider?.meta?.platformManaged;
  if (!managed || !account) return false;
  return (
    managed.origin === account.baseUrl && managed.userId === account.user.id
  );
}

/** 只保留当前登录账号的平台托管行；其他自建项、其他账号项原样保留在原数据里。 */
export function listAccountManagedProviders(
  providers: Record<string, Provider>,
  account: PlatformAccount | null | undefined,
): Record<string, Provider> {
  if (!account) return {};
  return Object.fromEntries(
    Object.entries(providers).filter(([, provider]) =>
      isAccountManaged(provider, account),
    ),
  );
}
