import { useCallback, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { Provider } from "@/types";
import type { AppId } from "@/lib/api/index";
import { providersApi } from "@/lib/api/providers";
import { useOpenClawLiveProviderIds } from "@/hooks/useOpenClaw";
import { useHermesLiveProviderIds } from "@/hooks/useHermes";
import { usePiCurrentState } from "@/lib/query/pi";
import { isPiFamilyApp } from "@/config/appConfig";

/**
 * 共存式应用里「这家在不在已添加列表」的统一判断：OpenCode / OpenClaw / Hermes 读各自的
 * live 名单，Pi 读原生配置状态，Kimi / MiniMax Code / WorkBuddy 看
 * DB 行上的 liveConfigManaged。供应商列表和页头的平台批量按钮共用这一份判断，别处不要再
 * 复制一份。`enabled=false` 时不发起查询（isPending 为 false），但已有缓存仍可用于展示。
 */
export function useConfigMembership(appId: AppId, enabled = true) {
  const {
    data: opencodeLiveIds,
    isPending: isOpencodeLiveIdsPending,
    isSuccess: isOpencodeLiveIdsSuccess,
  } = useQuery({
    queryKey: ["opencodeLiveProviderIds"],
    queryFn: () => providersApi.getOpenCodeLiveProviderIds(),
    enabled: enabled && appId === "opencode",
  });
  const {
    data: openclawLiveIds,
    isPending: isOpenclawLiveIdsPending,
    isSuccess: isOpenclawLiveIdsSuccess,
  } = useOpenClawLiveProviderIds(enabled && appId === "openclaw");
  const {
    data: hermesLiveIds,
    isPending: isHermesLiveIdsPending,
    isSuccess: isHermesLiveIdsSuccess,
  } = useHermesLiveProviderIds(enabled && appId === "hermes");
  const {
    data: piCurrentState,
    isSuccess: isPiCurrentStateSuccess,
    isPending: isPiCurrentStatePending,
  } = usePiCurrentState(
    enabled && isPiFamilyApp(appId),
    appId === "omp" ? "omp" : "pi",
  );
  // 列表和批量按钮会反复判断成员；原生状态变化时建立索引，避免每张卡片都线性扫描 ID 数组。
  const liveIds = useMemo(() => {
    const ids: Partial<Record<AppId, readonly string[] | undefined>> = {
      opencode: opencodeLiveIds,
      openclaw: openclawLiveIds,
      hermes: hermesLiveIds,
      [appId === "omp" ? "omp" : "pi"]: isPiCurrentStateSuccess
        ? piCurrentState?.enabledProviderIds
        : undefined,
    };
    return new Set(ids[appId]);
  }, [
    appId,
    opencodeLiveIds,
    openclawLiveIds,
    hermesLiveIds,
    isPiCurrentStateSuccess,
    piCurrentState,
  ]);

  const isInConfig = useCallback(
    (provider: Provider): boolean => {
      switch (appId) {
        case "kimi":
        case "mcode":
        case "workbuddy":
          return provider.meta?.liveConfigManaged === true;
        case "opencode":
        case "openclaw":
        case "hermes":
        case "pi":
        case "omp":
          return liveIds.has(provider.id);
        default:
          return true;
      }
    },
    [appId, liveIds],
  );

  // 累加式应用要等 live 里有哪些供应商读回来（成功或失败）才知道卡片该进哪个分区。
  // 禁用的查询 isPending 恒为 true，关闭时直接报 false，别让调用方一直等
  const isPending =
    enabled &&
    ((appId === "opencode" && isOpencodeLiveIdsPending) ||
      (appId === "openclaw" && isOpenclawLiveIdsPending) ||
      (appId === "hermes" && isHermesLiveIdsPending) ||
      (isPiFamilyApp(appId) && isPiCurrentStatePending));

  // A failed read is not an empty membership list: bulk writes require a successful snapshot.
  const isReady =
    enabled &&
    (
      {
        opencode: isOpencodeLiveIdsSuccess,
        openclaw: isOpenclawLiveIdsSuccess,
        hermes: isHermesLiveIdsSuccess,
        pi: isPiCurrentStateSuccess,
        omp: isPiCurrentStateSuccess,
      } as Partial<Record<AppId, boolean>>
    )[appId] !== false;

  return { isInConfig, isPending, isReady };
}
