import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Download, Loader2, Unplug } from "lucide-react";
import type { AppId } from "@/lib/api";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { providersQueryOptions, useSettingsQuery } from "@/lib/query/queries";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { extractErrorMessage } from "@/utils/errorUtils";
import { AppPageHeader } from "@/components/shell/AppPageHeader";
import { APP_DISPLAY_NAME, AppGlyph } from "@/components/shell/AppGlyph";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { usePlatformOperationScope } from "@/zhishu/auth/usePlatformOperationScope";
import {
  platformAuthError,
  type PlatformAccount,
} from "@/zhishu/api/platformAuth";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
import { PLATFORM_SYNC_APPS } from "@/zhishu/models/platformConfig";
import { usePlatformCatalog } from "@/zhishu/models/query/platformCatalog";
import { usePlatformProviderDeletion } from "@/zhishu/models/usePlatformProviderDeletion";
import {
  isAccountManaged,
  listAccountManagedProviders,
} from "@/zhishu/models/platformManaged";
import {
  fetchPlatformGroupData,
  pickPlatformGroupId,
  provisionPlatformApp,
  usePlatformAppInstalls,
  type PlatformGroupSelectionData,
  type PlatformProvisionOutcome,
} from "@/zhishu/models/platformProvision";
import {
  DEFAULT_VISIBLE_APPS,
  isModeAppId,
  isStackAppId,
  orderAppIds,
} from "@/config/appConfig";
import type { AppMode, AppModeView } from "@/types/proxy";

const APP_CONNECTION_STATUS = {
  notInstalled: {
    icon: Download,
    label: "platformOnboarding.notInstalled",
    tone: "bg-warning-soft text-warning-text",
  },
  notConnected: {
    icon: Unplug,
    label: "simpleMode.notConnected",
    tone: "bg-subtle text-fg-2",
  },
  connected: {
    icon: Check,
    label: "simpleMode.connected",
    tone: "bg-success-soft text-success-text",
  },
} as const;

/**
 * 简易模式的「应用」页：每个工具一行切换开关。
 *
 * 开 = 「接入到工具」向导确认按钮的同一套接入逻辑（模式应用按行内所选
 * 直连/路由/聚合应用，累加式加入全部平台供应商）；关 = 复用「一键删除平台
 * 供应商」，只移除当前账号的平台托管行，不触碰自建和其他账号的供应商。
 * 页面只读本机平台目录快照，不发平台请求、不创建/撤销 Key、不刷新额度。
 */
export function PlatformAppsPage({
  onOpenApp,
}: {
  onOpenApp: (app: AppId) => void;
}) {
  const { t } = useTranslation();
  const account = usePlatformAuth()?.account ?? null;
  const settings = useSettingsQuery().data;
  const catalog = usePlatformCatalog();
  const groups = catalog.data?.groups ?? [];
  const visibleApps = useMemo<Record<string, boolean>>(
    () => ({ ...DEFAULT_VISIBLE_APPS, ...settings?.visibleApps }),
    [settings?.visibleApps],
  );
  const apps = useMemo(
    () =>
      orderAppIds(settings?.appOrder).filter(
        (app) => PLATFORM_SYNC_APPS.has(app) && visibleApps[app],
      ),
    [settings?.appOrder, visibleApps],
  );
  const detection = usePlatformAppInstalls(apps);
  const groupOrder = useMemo(
    () => new Map(groups.map((group, index) => [group.id, index])),
    [groups],
  );

  if (!account) return null;

  return (
    <>
      <AppPageHeader title={t("simpleMode.title", "应用")} />
      <div
        id="main-content"
        className="min-h-0 flex-1 overflow-y-auto scroll-stable px-6 pb-12 pt-4"
      >
        <p className="mb-4 text-caption leading-relaxed text-fg-3">
          {t(
            "simpleMode.description",
            "为每个工具选择供应商。「智枢平台」的模型随账号自动同步。",
          )}
        </p>
        {catalog.isLoading && (
          <p role="status" className="mb-3 text-caption text-fg-3">
            {t("platformOnboarding.loadingCatalog")}
          </p>
        )}
        {!catalog.isLoading && groups.length === 0 && (
          <p role="alert" className="mb-3 text-caption text-warning-text">
            {t("platformOnboarding.noCatalog")}
          </p>
        )}
        {detection.isFetching && (
          <p
            role="status"
            className="mb-2 flex items-center gap-1.5 text-caption text-fg-3"
          >
            <Loader2 className="h-3 w-3 animate-spin" />
            {t("platformOnboarding.detecting")}
          </p>
        )}
        {apps.length === 0 ? (
          <p className="text-caption text-fg-3">
            {t("platformOnboarding.noApps")}
          </p>
        ) : (
          <ul className="divide-y divide-border rounded-control border border-border">
            {apps.map((app) => (
              <PlatformAppRow
                key={app}
                app={app}
                account={account}
                groupOrder={groupOrder}
                installed={detection.isSuccess ? detection.data.has(app) : null}
                onOpenApp={onOpenApp}
              />
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

function PlatformAppRow({
  app,
  account,
  groupOrder,
  installed,
  onOpenApp,
}: {
  app: AppId;
  account: PlatformAccount;
  /** 目录分组 id → 目录顺序；与「接入到工具」向导共用一份排序。 */
  groupOrder: Map<number, number>;
  /** 安装检测结果；检测中为 null（不禁用，按已安装处理）。 */
  installed: boolean | null;
  onOpenApp: (app: AppId) => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const captureScope = usePlatformOperationScope(account, "platform-apps");
  const name = APP_DISPLAY_NAME[app];

  const providersQuery = useQuery(providersQueryOptions(app));
  const providers = providersQuery.data?.providers ?? {};
  const currentProviderId = providersQuery.data?.currentProviderId ?? "";
  const managed = useMemo(
    () => listAccountManagedProviders(providers, account),
    [providers, account],
  );
  const connected = Object.keys(managed).length > 0;

  // 行内选择；用户没动过就按推荐默认值（当前模式 / 当前生效托管分组）。
  const isModeApp = isModeAppId(app);
  const [selectedMode, setSelectedMode] = useState<AppMode>();
  const [selectedGroup, setSelectedGroup] = useState<number>();
  const modeQuery = useQuery({
    queryKey: ["providers", app, "mode"] as const,
    queryFn: () => proxyApi.getAppMode(app),
    enabled: isModeApp,
    staleTime: 30_000,
    retry: false,
  });
  const groupQuery = useQuery({
    queryKey: [
      "platform-app-groups",
      account.baseUrl,
      account.user.id,
      app,
    ] as const,
    queryFn: () => fetchPlatformGroupData(app, account),
    enabled: isModeApp,
    staleTime: 30_000,
    retry: false,
  });
  const modeView: AppModeView | undefined = isModeApp
    ? modeQuery.data
    : undefined;
  const mode = selectedMode ?? modeView?.mode ?? "direct";
  const groupId = pickPlatformGroupId(selectedGroup, groupQuery.data, modeView);
  const groupOptions: PlatformGroupSelectionData["options"] =
    groupQuery.data?.options ?? [];
  const appModes: AppMode[] = isStackAppId(app)
    ? ["direct", "route", "stack"]
    : ["direct", "route"];
  // 模式应用要等分组选项可用；累加式应用没有分组概念
  const groupsReady =
    !isModeApp || (!groupQuery.isLoading && groupOptions.length > 0);

  const provision = useMutation({
    mutationKey: ["platformProvisionApp", app],
    mutationFn: async () => {
      const isCurrent = captureScope();
      const outcome = await provisionPlatformApp(app, {
        t,
        account,
        groupOrder,
        groupId,
        mode,
        isCurrent,
      });
      if (!outcome || !isCurrent()) return null;
      await refreshPlatformProviders(queryClient, app);
      if (!isCurrent()) return null;
      try {
        await providersApi.updateTrayMenu();
      } catch {
        // 托盘刷新失败不影响已完成的配置写入。
      }
      return outcome;
    },
    onSuccess: (outcome) => {
      if (!outcome) return;
      if (!outcome.ok) {
        toast.error(
          t("platformOnboarding.resultFailed", {
            message: outcome.message ?? "",
          }),
        );
        return;
      }
      const text = provisionResultText(outcome, t);
      if (outcome.message) {
        toast.warning(text, {
          description: outcome.message,
          closeButton: true,
        });
      } else {
        toast.success(text);
      }
    },
    onError: (cause) => {
      toast.error(extractErrorMessage(cause) || platformAuthError(cause));
    },
  });

  // 关 = 复用「一键删除平台供应商」：只删当前账号在该应用的平台托管行，
  // 自建项和其他账号项不动；身份快照、缓存失效与 platform_delete 报告都在 hook 内。
  const deletion = usePlatformProviderDeletion(app, managed, { notify: true });

  const currentProvider = currentProviderId
    ? providers[currentProviderId]
    : undefined;
  const status =
    installed === false
      ? "notInstalled"
      : connected
        ? "connected"
        : "notConnected";
  const { icon: StatusIcon, label, tone } = APP_CONNECTION_STATUS[status];
  let detail: string | undefined;
  if (status === "connected") {
    if (currentProvider && isAccountManaged(currentProvider, account)) {
      const model = currentProvider.meta?.platformManaged?.model;
      detail = model
        ? `${currentProvider.name},${model}`
        : currentProvider.name;
    } else {
      detail = t("simpleMode.inactiveHint");
    }
  }

  return (
    <li className="flex items-center gap-3 px-3 py-2.5">
      <button
        type="button"
        onClick={() => onOpenApp(app)}
        className="flex min-w-0 flex-1 items-center gap-3 rounded-control text-start transition-colors hover:opacity-80"
      >
        <AppGlyph app={app} size={20} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-body font-medium text-fg-1">
            {name}
          </span>
          <span className="mt-0.5 flex min-w-0 items-center gap-2 text-caption">
            <span
              className={cn(
                "inline-flex shrink-0 items-center gap-1 whitespace-nowrap rounded-control px-1.5 py-0.5 font-medium",
                tone,
              )}
            >
              <StatusIcon aria-hidden="true" className="size-3" />
              {t(label)}
            </span>
            {detail && (
              <span className="truncate text-fg-3" title={detail}>
                {detail}
              </span>
            )}
          </span>
        </span>
      </button>
      {isModeApp ? (
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
          <Select
            value={groupId !== undefined ? String(groupId) : ""}
            onValueChange={(value) => setSelectedGroup(Number(value))}
            disabled={provision.isPending || groupOptions.length === 0}
          >
            <SelectTrigger
              className="h-7 w-[220px] text-caption"
              aria-label={t("platformOnboarding.groupLabel", { name })}
              title={
                mode === "stack"
                  ? t("platformOnboarding.stackDefaultHint")
                  : undefined
              }
            >
              <SelectValue
                placeholder={
                  groupQuery.isLoading
                    ? t("platformOnboarding.groupLoading")
                    : t("platformOnboarding.noGroupsForApp")
                }
              />
            </SelectTrigger>
            <SelectContent>
              {groupOptions.map((option) => (
                <SelectItem key={option.groupId} value={String(option.groupId)}>
                  {t("platformOnboarding.groupOption", {
                    name: option.groupName,
                    count: option.modelCount,
                  })}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={mode}
            onValueChange={(value) => setSelectedMode(value as AppMode)}
            disabled={provision.isPending}
          >
            <SelectTrigger
              className="h-7 w-[104px] text-caption"
              aria-label={t("platformOnboarding.modeLabel", { name })}
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {appModes.map((appMode) => (
                <SelectItem key={appMode} value={appMode}>
                  {t(`platformOnboarding.mode.${appMode}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      ) : (
        <span className="shrink-0 text-caption text-fg-3">
          {t("platformOnboarding.additiveMode")}
        </span>
      )}
      {provision.isPending && (
        <Loader2
          aria-hidden="true"
          className="h-3.5 w-3.5 shrink-0 animate-spin text-fg-3"
        />
      )}
      <Switch
        checked={connected}
        disabled={
          installed === false ||
          provision.isPending ||
          deletion.pending ||
          (isModeApp && !connected && !groupsReady)
        }
        onCheckedChange={(checked) => {
          if (checked) provision.mutate();
          else deletion.remove();
        }}
        aria-label={
          connected
            ? t("simpleMode.switchOffLabel", { name })
            : t("simpleMode.switchOnLabel", { name })
        }
      />
    </li>
  );
}

/** 与「接入到工具」向导的结果文案共用同一组 key。 */
function provisionResultText(
  outcome: PlatformProvisionOutcome,
  t: ReturnType<typeof useTranslation>["t"],
) {
  switch (outcome.action) {
    case "direct":
      return t("platformOnboarding.resultDirect", {
        provider: outcome.providerName ?? "",
      });
    case "route":
      return t("platformOnboarding.resultRoute", {
        provider: outcome.providerName ?? "",
      });
    case "stack":
      return t("platformOnboarding.resultStack", {
        provider: outcome.providerName ?? "",
        count: outcome.count ?? 0,
      });
    case "additive":
      return t("platformOnboarding.resultAdditive", {
        count: outcome.count ?? 0,
      });
    default:
      return t("platformOnboarding.resultSuccess", {
        provider: outcome.providerName ?? "",
      });
  }
}
