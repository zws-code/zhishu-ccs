import { useMemo } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ListMinus, ListPlus, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Provider } from "@/types";
import type { AppMode } from "@/types/proxy";
import type { AppId } from "@/lib/api";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { usePlatformAuth } from "@/contexts/PlatformAuthContext";
import { isAdditiveAppId, isStackAppId } from "@/config/appConfig";
import { useConfigMembership } from "@/hooks/useConfigMembership";
import { useSwitchProviderMutation } from "@/lib/query/mutations";
import { refreshAdditiveLiveState } from "@/lib/query/additiveLiveState";
import { useProxyStack } from "@/lib/query/proxy";
import { Button } from "@/components/ui/button";
import { toast } from "@/lib/toast";
import { extractErrorMessage } from "@/utils/errorUtils";
import { PLATFORM_SYNC_APPS } from "@/config/platformConfig";

/**
 * 「同步平台供应商」旁边的一键添加 / 移除，只作用于当前账户的平台托管供应商：
 * 共存式应用对着「已添加 / 可添加」两个分区常驻显示；模式应用只在聚合页签显示，
 * 直连 / 路由没有这个功能。移除只是移出配置 / 名单，供应商行保留在「可添加」里。
 */
export function PlatformBulkSyncButtons({
  app,
  modeView,
  providers,
}: {
  app: AppId;
  /** 模式应用正在查看的模式；非模式应用传 null */
  modeView: AppMode | null;
  providers: Provider[];
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const auth = usePlatformAuth();
  const account = auth?.account;
  const inStackView = modeView === "stack" && isStackAppId(app);
  const applies =
    Boolean(account) &&
    PLATFORM_SYNC_APPS.has(app) &&
    (isAdditiveAppId(app) || inStackView);

  // 跨账号残留的托管行不属于当前账户，不参与批量添加 / 移除
  const platformProviders = useMemo(
    () =>
      account
        ? providers.filter(
            (p) =>
              p.meta?.platformManaged?.origin === account.baseUrl &&
              p.meta?.platformManaged?.userId === account.user.id,
          )
        : [],
    [providers, account],
  );
  // 不满足显示条件时一个查询都不发：页头在供应商列表之外，这里的查询必须
  // 纯属「有平台供应商可操作」的副产品，不能干扰列表自己的成员查询
  const active = applies && platformProviders.length > 0;

  const { isInConfig, isPending: membershipPending } = useConfigMembership(
    app,
    active && isAdditiveAppId(app),
  );
  const { data: stack } = useProxyStack(app, active && inStackView);

  const membershipReady = !isAdditiveAppId(app) || !membershipPending;
  const stackReady = !inStackView || Boolean(stack);
  const memberIds = useMemo(
    () => new Set((stack?.members ?? []).map((member) => member.providerId)),
    [stack?.members],
  );
  // 用同一次成员判断划分添加与移除目标，保持列表顺序；查询尚未结束时不允许批量写入。
  const { addTargets, removeTargets } = useMemo(() => {
    const addTargets: Provider[] = [];
    const removeTargets: Provider[] = [];
    if (membershipReady && stackReady) {
      for (const provider of platformProviders) {
        const included = inStackView
          ? memberIds.has(provider.id)
          : isInConfig(provider);
        (included ? removeTargets : addTargets).push(provider);
      }
    }
    return { addTargets, removeTargets };
  }, [
    membershipReady,
    stackReady,
    platformProviders,
    inStackView,
    memberIds,
    isInConfig,
  ]);

  const switchMutation = useSwitchProviderMutation(app);
  const bulk = useMutation({
    mutationFn: async (action: "add" | "remove") => {
      const targets = action === "add" ? addTargets : removeTargets;
      const failures: string[] = [];
      const notices = new Set<string>();
      for (const provider of targets) {
        try {
          if (isAdditiveAppId(app)) {
            if (action === "add") {
              // Kimi 的「添加」只写 config.toml、不动默认模型，和「切换」是两条路
              if (app === "kimi") {
                await providersApi.addToLiveConfig(provider.id, app);
              } else {
                await switchMutation.mutateAsync(provider.id);
              }
            } else {
              await providersApi.removeFromLiveConfig(provider.id, app);
            }
          } else {
            const notice = await proxyApi.setProxyStackMember(
              app,
              provider.id,
              action === "add",
            );
            if (notice) notices.add(notice);
          }
        } catch (error) {
          failures.push(
            `${provider.name}: ${
              extractErrorMessage(error) || t("common.unknown")
            }`,
          );
        }
      }
      return { action, total: targets.length, failures, notices: [...notices] };
    },
    onSuccess: async ({ action, total, failures, notices }) => {
      if (isAdditiveAppId(app)) {
        // switch 路径的每次成功调用已自带各自的缓存失效，这里补移除路径和 Kimi 添加
        await refreshAdditiveLiveState(queryClient, app);
      } else {
        await queryClient.invalidateQueries({ queryKey: ["providers", app] });
      }
      if (total === 0) return;
      const done = total - failures.length;
      if (failures.length === 0) {
        toast.success(
          t(
            action === "add"
              ? "platformCatalog.bulkAddDone"
              : "platformCatalog.bulkRemoveDone",
            {
              count: done,
              defaultValue:
                action === "add"
                  ? "已添加 {{count}} 个平台供应商"
                  : "已移除 {{count}} 个平台供应商",
            },
          ),
        );
      } else {
        toast.warning(
          t("platformCatalog.bulkPartial", {
            success: done,
            failed: failures.length,
            defaultValue: "{{success}} 个成功，{{failed}} 个失败",
          }),
          { description: failures.slice(0, 3).join("\n"), closeButton: true },
        );
      }
      for (const notice of notices) {
        toast.warning(t(`provider.${notice}`), { closeButton: true });
      }
    },
  });

  if (!active) return null;

  const busyAction = bulk.variables;
  const addHint = t(
    inStackView
      ? "platformCatalog.bulkAddStackHint"
      : "platformCatalog.bulkAddHint",
    {
      defaultValue: inStackView
        ? "将所有平台供应商加入聚合名单"
        : "将所有未添加的平台供应商加入已添加列表",
    },
  );
  const removeHint = t(
    inStackView
      ? "platformCatalog.bulkRemoveStackHint"
      : "platformCatalog.bulkRemoveHint",
    {
      defaultValue: inStackView
        ? "将聚合名单中的平台供应商移出名单"
        : "将已添加的平台供应商从配置中移除，保留在可添加列表",
    },
  );

  return (
    <>
      <Button
        variant="outline"
        size="regular"
        disabled={bulk.isPending || addTargets.length === 0}
        aria-busy={bulk.isPending && busyAction === "add"}
        title={addHint}
        onClick={() => bulk.mutate("add")}
      >
        {bulk.isPending && busyAction === "add" ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <ListPlus className="h-4 w-4" />
        )}
        {t("platformCatalog.bulkAdd", { defaultValue: "一键添加" })}
      </Button>
      <Button
        variant="outline"
        size="regular"
        disabled={bulk.isPending || removeTargets.length === 0}
        aria-busy={bulk.isPending && busyAction === "remove"}
        title={removeHint}
        onClick={() => bulk.mutate("remove")}
      >
        {bulk.isPending && busyAction === "remove" ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <ListMinus className="h-4 w-4" />
        )}
        {t("platformCatalog.bulkRemove", { defaultValue: "一键移除" })}
      </Button>
    </>
  );
}
