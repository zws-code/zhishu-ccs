import type { QueryClient } from "@tanstack/react-query";
import type { AppId } from "@/lib/api";

/**
 * 本地供应商同步后的统一缓存刷新入口，登录自动同步与手动同步共用。
 * 保持各工具原生状态、代理和历史记录的刷新范围；不失效平台目录或额度，避免隐式联网。
 */
export async function refreshPlatformProviders(
  queryClient: QueryClient,
  agent?: AppId,
) {
  await Promise.all(
    [
      "providers",
      "proxyStatus",
      "proxyTakeoverStatus",
      "opencodeLiveProviderIds",
      "opencodeDefaultModel",
      "opencode",
      "openclaw",
      "hermes",
      "pi",
      "dsh",
      "claude-desktop-status",
      "mcode",
      "kimi",
      "kimi-oauth-status",
      "configChangeHistory",
    ].map((key) =>
      queryClient.invalidateQueries({
        queryKey: key === "providers" && agent ? [key, agent] : [key],
      }),
    ),
  );
}
