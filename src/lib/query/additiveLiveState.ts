import type { QueryClient } from "@tanstack/react-query";
import type { AppId } from "@/lib/api";
import { openclawKeys } from "@/hooks/useOpenClaw";
import { hermesKeys } from "@/hooks/useHermes";
import { invalidateDshProviderCaches } from "@/lib/query/dsh";
import { invalidatePiProviderCaches } from "@/lib/query/pi";

/**
 * 累加式应用的「已添加」和「默认」都现读客户端文件，增删供应商前后重读一遍。
 * 供应商页的移除流程和页头的平台批量按钮共用。
 */
export async function refreshAdditiveLiveState(
  queryClient: QueryClient,
  app: AppId,
) {
  if (app === "pi") {
    await invalidatePiProviderCaches(queryClient);
  } else if (app === "opencode") {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: ["opencodeLiveProviderIds"],
      }),
      queryClient.invalidateQueries({ queryKey: ["opencodeDefaultModel"] }),
    ]);
  } else if (app === "openclaw") {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: openclawKeys.liveProviderIds,
      }),
      queryClient.invalidateQueries({ queryKey: openclawKeys.defaultModel }),
      queryClient.invalidateQueries({ queryKey: openclawKeys.health }),
    ]);
  } else if (app === "hermes") {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: hermesKeys.liveProviderIds }),
      queryClient.invalidateQueries({ queryKey: hermesKeys.modelConfig }),
    ]);
  } else if (app === "mcode" || app === "kimi") {
    await queryClient.invalidateQueries({ queryKey: ["providers", app] });
  } else if (app === "deepseek-harness") {
    await invalidateDshProviderCaches(queryClient);
  } else if (app === "workbuddy") {
    await queryClient.invalidateQueries({
      queryKey: ["providers", "workbuddy"],
    });
  }
}
