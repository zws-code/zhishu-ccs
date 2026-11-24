import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { CheckCircle2, Loader2, PlugZap, X } from "lucide-react";
import type { AppId } from "@/lib/api";
import { settingsApi } from "@/lib/api/index";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { useSettingsQuery } from "@/lib/query/queries";
import { toast } from "@/lib/toast";
import { APP_DISPLAY_NAME, AppGlyph } from "@/components/shell/AppGlyph";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  platformAuthError,
  type PlatformAccount,
} from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { usePlatformOperationScope } from "@/zhishu/auth/usePlatformOperationScope";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
import { PLATFORM_SYNC_APPS } from "@/zhishu/models/platformConfig";
import { usePlatformCatalog } from "@/zhishu/models/query/platformCatalog";
import {
  fetchPlatformGroupData,
  pickPlatformGroupId,
  provisionPlatformApp,
  usePlatformAppInstalls,
  type PlatformProvisionOutcome,
} from "@/zhishu/models/platformProvision";
import {
  DEFAULT_VISIBLE_APPS,
  isModeAppId,
  isStackAppId,
  orderAppIds,
} from "@/config/appConfig";
import type { Settings, VisibleApps } from "@/types";
import type { AppMode } from "@/types/proxy";

const DISMISSED_PREFIX = "zhishu-platform-onboarding-dismissed:";

/** 每个账号只自动打开一次；关闭或完成接入后不再自动出现，仍可从平台基础数据页手动打开。 */
function dismissalKey(baseUrl: string, userId: number) {
  return `${DISMISSED_PREFIX}${encodeURIComponent(baseUrl)}:${userId}`;
}

function isDismissed(baseUrl: string, userId: number) {
  try {
    return window.localStorage.getItem(dismissalKey(baseUrl, userId)) === "1";
  } catch {
    return false;
  }
}

function markDismissed(baseUrl: string, userId: number) {
  try {
    window.localStorage.setItem(dismissalKey(baseUrl, userId), "1");
  } catch {
    // 隐私模式下写不了 localStorage 时只是下次会再自动打开，不影响接入。
  }
}

type OnboardingResult = PlatformProvisionOutcome;

interface PlatformOnboardingValue {
  open: () => void;
}

const PlatformOnboardingContext = createContext<PlatformOnboardingValue | null>(
  null,
);

export function usePlatformOnboarding() {
  return useContext(PlatformOnboardingContext);
}

/** 登录后只读目录；接入由这个向导在用户确认后执行，不在登录时静默改写供应商列表。 */
export function PlatformOnboardingProvider({
  children,
}: {
  children: ReactNode;
}) {
  const auth = usePlatformAuth();
  const account = auth?.account ?? null;
  // 只读缓存，不额外发 IPC；目录由登录后的自动拉取或手动同步写入。
  const cachedCatalog = usePlatformCatalog(false);
  // 同样只读缓存：首次运行提示还没确认时不自动打开，避免两个对话框叠在一起。
  const cachedSettings = useQuery<Settings>({
    queryKey: ["settings"],
    queryFn: () => settingsApi.get(),
    enabled: false,
    staleTime: Infinity,
  });
  const [open, setOpen] = useState(false);
  const autoOpened = useRef<string | null>(null);
  const identity = account
    ? JSON.stringify([account.baseUrl, account.user.id])
    : null;

  useEffect(() => {
    setOpen(false);
  }, [identity]);

  useEffect(() => {
    if (!account || !identity) return;
    if (autoOpened.current === identity) return;
    if (cachedSettings.data?.firstRunNoticeConfirmed !== true) return;
    const groups = cachedCatalog.data?.groups;
    if (!groups || groups.length === 0) return;
    if (isDismissed(account.baseUrl, account.user.id)) return;
    autoOpened.current = identity;
    setOpen(true);
  }, [account, identity, cachedSettings.data, cachedCatalog.data]);

  const openWizard = useCallback(() => setOpen(true), []);
  const value = useMemo(() => ({ open: openWizard }), [openWizard]);

  return (
    <PlatformOnboardingContext.Provider value={value}>
      {children}
      {account && open && (
        <PlatformOnboardingDialog
          account={account}
          onClose={() => {
            markDismissed(account.baseUrl, account.user.id);
            setOpen(false);
          }}
        />
      )}
    </PlatformOnboardingContext.Provider>
  );
}

function PlatformOnboardingDialog({
  account,
  onClose,
}: {
  account: PlatformAccount;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const captureScope = usePlatformOperationScope(
    account,
    "platform-onboarding",
  );
  const settings = useSettingsQuery().data;
  const catalog = usePlatformCatalog();
  const groups = catalog.data?.groups ?? [];
  const visibleApps = useMemo<VisibleApps>(
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

  // 模式应用先读当前模式作为默认值；累加式应用没有直连/路由/聚合的概念。
  const modeApps = useMemo<AppId[]>(
    () => apps.filter((app) => isModeAppId(app)),
    [apps],
  );
  const modeQueries = useQueries({
    queries: modeApps.map((app) => ({
      queryKey: ["providers", app, "mode"] as const,
      queryFn: () => proxyApi.getAppMode(app),
      staleTime: 30_000,
      retry: false,
    })),
  });
  const [modes, setModes] = useState<Map<AppId, AppMode>>(new Map());
  const touchedModes = useRef(new Set<AppId>());

  useEffect(() => {
    setModes((previous) => {
      let changed = false;
      const next = new Map(previous);
      modeApps.forEach((app, index) => {
        if (touchedModes.current.has(app)) return;
        const mode = modeQueries[index]?.data?.mode;
        if (mode && next.get(app) !== mode) {
          next.set(app, mode);
          changed = true;
        }
      });
      return changed ? next : previous;
    });
  }, [modeApps, modeQueries]);

  const modeFor = (app: AppId): AppMode => {
    const chosen = modes.get(app);
    if (chosen) return chosen;
    const index = modeApps.indexOf(app);
    return modeQueries[index]?.data?.mode ?? "direct";
  };
  const setModeFor = (app: AppId, mode: AppMode) => {
    touchedModes.current.add(app);
    setModes((previous) => new Map(previous).set(app, mode));
  };

  // 分组选项只读预览，仅模式应用需要；累加式应用直接加入全部平台供应商。
  const groupQueries = useQueries({
    queries: modeApps.map((app) => ({
      queryKey: [
        "platform-onboarding-groups",
        account.baseUrl,
        account.user.id,
        app,
      ] as const,
      queryFn: () => fetchPlatformGroupData(app, account),
      staleTime: 30_000,
      retry: false,
    })),
  });
  const groupQueryFor = (app: AppId) => groupQueries[modeApps.indexOf(app)];
  const groupOptionsFor = (app: AppId) =>
    groupQueryFor(app)?.data?.options ?? [];
  const [selectedGroups, setSelectedGroups] = useState<Map<AppId, number>>(
    new Map(),
  );
  const groupFor = (app: AppId): number | undefined =>
    pickPlatformGroupId(
      selectedGroups.get(app),
      groupQueryFor(app)?.data,
      modeApps.includes(app)
        ? modeQueries[modeApps.indexOf(app)]?.data
        : undefined,
    );
  const setGroupFor = (app: AppId, groupId: number) => {
    setSelectedGroups((previous) => new Map(previous).set(app, groupId));
  };

  const [selected, setSelected] = useState<Set<AppId>>(new Set());
  const [results, setResults] = useState<OnboardingResult[] | null>(null);
  const touched = useRef(false);

  // 检测完成后默认勾选已安装的工具；用户手动改过就不再覆盖。
  useEffect(() => {
    if (touched.current || !detection.isSuccess) return;
    setSelected(new Set(detection.data ?? []));
  }, [detection.isSuccess, detection.data]);

  const toggle = (app: AppId, checked: boolean) => {
    touched.current = true;
    setSelected((previous) => {
      const next = new Set(previous);
      if (checked) next.add(app);
      else next.delete(app);
      return next;
    });
  };

  const selectAll = () => {
    touched.current = true;
    setSelected(new Set(apps));
  };
  const clearAll = () => {
    touched.current = true;
    setSelected(new Set());
  };

  const provision = useMutation({
    mutationKey: ["platformOnboarding"],
    mutationFn: async ({
      targets,
      isCurrent,
    }: {
      targets: AppId[];
      isCurrent: () => boolean;
    }) => {
      const outcomes: OnboardingResult[] = [];
      const groupOrder = new Map(
        groups.map((group, index) => [group.id, index]),
      );
      for (const app of targets) {
        if (!isCurrent()) return null;
        const outcome = await provisionPlatformApp(app, {
          t,
          account,
          groupOrder,
          groupId: groupFor(app),
          mode: modeFor(app),
          isCurrent,
        });
        // 中途账号切换或对话框关闭：放弃剩余应用，已完成的结果一并丢弃
        if (!outcome) return null;
        outcomes.push(outcome);
      }
      return outcomes;
    },
    onSuccess: async (outcomes, { isCurrent }) => {
      if (!outcomes || !isCurrent()) return;
      setResults(outcomes);
      await refreshPlatformProviders(queryClient);
      if (!isCurrent()) return;
      try {
        await providersApi.updateTrayMenu();
      } catch {
        // 托盘刷新失败不影响已完成的配置写入。
      }
      const failed = outcomes.filter((outcome) => !outcome.ok).length;
      const summary = {
        success: outcomes.length - failed,
        failed,
      };
      if (failed) toast.warning(t("platformOnboarding.resultSummary", summary));
      else toast.success(t("platformOnboarding.resultSummary", summary));
      markDismissed(account.baseUrl, account.user.id);
    },
    onError: (cause, { isCurrent }) => {
      if (!isCurrent()) return;
      toast.error(platformAuthError(cause));
    },
  });

  const pending = provision.isPending;
  const failedResults = results?.filter((result) => !result.ok) ?? [];
  const selectedApps = apps.filter((app) => selected.has(app));
  // 当前模式还没读回来时先不让确认，避免把路由/聚合中的应用误按直连接入。
  const modesLoading = selectedApps.some(
    (app) =>
      isModeAppId(app) &&
      modeQueries[modeApps.indexOf(app)]?.isLoading === true,
  );
  // 模式应用都要有可用的分组选项；累加式应用不选分组，直接加入全部平台供应商。
  const groupsReady = selectedApps.every(
    (app) =>
      !isModeAppId(app) ||
      (groupQueryFor(app)?.isLoading !== true &&
        groupOptionsFor(app).length > 0),
  );
  const canConnect =
    !pending &&
    !modesLoading &&
    groupsReady &&
    groups.length > 0 &&
    apps.length > 0 &&
    selected.size > 0;
  const resultText = (result: OnboardingResult) => {
    if (!result.ok) {
      return t("platformOnboarding.resultFailed", {
        message: result.message ?? "",
      });
    }
    switch (result.action) {
      case "direct":
        return t("platformOnboarding.resultDirect", {
          provider: result.providerName ?? "",
        });
      case "route":
        return t("platformOnboarding.resultRoute", {
          provider: result.providerName ?? "",
        });
      case "stack":
        return t("platformOnboarding.resultStack", {
          provider: result.providerName ?? "",
          count: result.count ?? 0,
        });
      case "additive":
        return t("platformOnboarding.resultAdditive", {
          count: result.count ?? 0,
        });
      default:
        return t("platformOnboarding.resultSuccess", {
          provider: result.providerName ?? "",
        });
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next && !pending) onClose();
      }}
    >
      <DialogContent className="w-[calc(100vw-2rem)] max-w-[680px] overflow-hidden">
        <DialogClose asChild>
          <button
            type="button"
            aria-label={t("common.close")}
            disabled={pending}
            className="absolute right-4 top-4 rounded-control p-1 text-fg-3 transition-colors hover:bg-subtle hover:text-fg-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40"
          >
            <X className="h-4 w-4" />
          </button>
        </DialogClose>
        <DialogHeader className="gap-2 pb-4 pt-6">
          <div className="flex h-10 w-10 items-center justify-center rounded-full bg-subtle text-fg-1">
            {results ? (
              <CheckCircle2 className="h-5 w-5" />
            ) : (
              <PlugZap className="h-5 w-5" />
            )}
          </div>
          <DialogTitle>
            {results
              ? t("platformOnboarding.resultTitle")
              : t("platformOnboarding.title")}
          </DialogTitle>
          <DialogDescription className="leading-relaxed">
            {results
              ? t("platformOnboarding.resultSummary", {
                  success: (results?.length ?? 0) - failedResults.length,
                  failed: failedResults.length,
                })
              : t("platformOnboarding.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-5">
          {results ? (
            <ul className="divide-y divide-border rounded-control border border-border">
              {results.map((result) => (
                <li
                  key={result.app}
                  className="flex items-start gap-3 px-3 py-2.5"
                >
                  <AppGlyph app={result.app} size={18} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-body font-medium text-fg-1">
                      {APP_DISPLAY_NAME[result.app]}
                    </p>
                    <p
                      className={
                        result.ok
                          ? "break-words text-caption text-fg-2"
                          : "break-words text-caption text-warning-text"
                      }
                    >
                      {result.ok
                        ? resultText(result)
                        : t("platformOnboarding.resultFailed", {
                            message: result.message ?? "",
                          })}
                    </p>
                    {result.ok && result.message && (
                      <p className="mt-1 break-words text-caption text-fg-3">
                        {result.message}
                      </p>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <>
              <p className="mb-3 rounded-control bg-warning-soft px-3 py-2 text-caption leading-relaxed text-warning-text">
                {t("platformOnboarding.warning")}
              </p>
              <p className="mb-3 text-caption leading-relaxed text-fg-3">
                {t("platformOnboarding.modeHint")}
              </p>
              {detection.isFetching && (
                <p
                  role="status"
                  className="mb-2 flex items-center gap-1.5 text-caption text-fg-3"
                >
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t("platformOnboarding.detecting")}
                </p>
              )}
              {detection.isError && (
                <p className="mb-2 text-caption text-fg-3">
                  {t("platformOnboarding.detectFailed")}
                </p>
              )}
              {catalog.isLoading && (
                <p role="status" className="text-caption text-fg-3">
                  {t("platformOnboarding.loadingCatalog")}
                </p>
              )}
              {!catalog.isLoading && groups.length === 0 && (
                <p role="alert" className="text-caption text-warning-text">
                  {t("platformOnboarding.noCatalog")}
                </p>
              )}
              {apps.length === 0 ? (
                <p className="text-caption text-fg-3">
                  {t("platformOnboarding.noApps")}
                </p>
              ) : (
                <>
                  <div className="mb-2 flex items-center justify-between gap-2 text-caption text-fg-3">
                    <span>
                      {t("platformOnboarding.groupsAvailable", {
                        count: groups.length,
                      })}
                    </span>
                    <span className="flex items-center gap-2">
                      <button
                        type="button"
                        className="hover:text-fg-1"
                        onClick={selectAll}
                        disabled={pending}
                      >
                        {t("platformOnboarding.selectAll")}
                      </button>
                      <button
                        type="button"
                        className="hover:text-fg-1"
                        onClick={clearAll}
                        disabled={pending}
                      >
                        {t("platformOnboarding.clearAll")}
                      </button>
                    </span>
                  </div>
                  <ul className="divide-y divide-border rounded-control border border-border">
                    {apps.map((app) => {
                      const installed = detection.isSuccess
                        ? (detection.data?.has(app) ?? false)
                        : null;
                      const appModes: AppMode[] = isStackAppId(app)
                        ? ["direct", "route", "stack"]
                        : ["direct", "route"];
                      const groupOptions = groupOptionsFor(app);
                      const groupQuery = groupQueryFor(app);
                      const groupValue = groupFor(app);
                      const mode = modeFor(app);
                      return (
                        <li
                          key={app}
                          className="flex flex-wrap items-center gap-3 px-3 py-2.5"
                        >
                          <Checkbox
                            checked={selected.has(app)}
                            onCheckedChange={(checked) => toggle(app, checked)}
                            disabled={pending}
                            aria-label={APP_DISPLAY_NAME[app]}
                          />
                          <AppGlyph app={app} size={18} />
                          <div className="flex min-w-0 flex-1 items-center gap-2">
                            <p className="truncate text-body font-medium text-fg-1">
                              {APP_DISPLAY_NAME[app]}
                            </p>
                            {installed !== null && (
                              <span className="shrink-0 text-caption text-fg-3">
                                {installed
                                  ? t("platformOnboarding.installed")
                                  : t("platformOnboarding.notInstalled")}
                              </span>
                            )}
                          </div>
                          {isModeAppId(app) ? (
                            <div className="flex flex-wrap items-center gap-2">
                              <Select
                                value={
                                  groupValue !== undefined
                                    ? String(groupValue)
                                    : ""
                                }
                                onValueChange={(value) =>
                                  setGroupFor(app, Number(value))
                                }
                                disabled={pending || groupOptions.length === 0}
                              >
                                <SelectTrigger
                                  className="h-7 w-[260px] text-caption"
                                  aria-label={t(
                                    "platformOnboarding.groupLabel",
                                    { name: APP_DISPLAY_NAME[app] },
                                  )}
                                  title={
                                    mode === "stack"
                                      ? t("platformOnboarding.stackDefaultHint")
                                      : undefined
                                  }
                                >
                                  <SelectValue
                                    placeholder={
                                      groupQuery?.isLoading
                                        ? t("platformOnboarding.groupLoading")
                                        : t("platformOnboarding.noGroupsForApp")
                                    }
                                  />
                                </SelectTrigger>
                                <SelectContent>
                                  {groupOptions.map((option) => (
                                    <SelectItem
                                      key={option.groupId}
                                      value={String(option.groupId)}
                                    >
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
                                onValueChange={(value) =>
                                  setModeFor(app, value as AppMode)
                                }
                                disabled={pending}
                              >
                                <SelectTrigger
                                  className="h-7 w-[104px] text-caption"
                                  aria-label={t(
                                    "platformOnboarding.modeLabel",
                                    { name: APP_DISPLAY_NAME[app] },
                                  )}
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
                        </li>
                      );
                    })}
                  </ul>
                </>
              )}
            </>
          )}
        </div>

        <DialogFooter className="border-t border-border py-4">
          {results ? (
            <>
              {failedResults.length > 0 && (
                <Button
                  variant="neutral"
                  disabled={pending}
                  onClick={() => {
                    setResults(null);
                    provision.mutate({
                      targets: failedResults.map((result) => result.app),
                      isCurrent: captureScope(),
                    });
                  }}
                >
                  {t("platformOnboarding.retry")}
                </Button>
              )}
              <Button onClick={onClose} disabled={pending}>
                {t("platformOnboarding.done")}
              </Button>
            </>
          ) : (
            <>
              <Button variant="neutral" onClick={onClose} disabled={pending}>
                {t("platformOnboarding.later")}
              </Button>
              <Button
                disabled={!canConnect}
                aria-busy={pending}
                onClick={() =>
                  provision.mutate({
                    targets: selectedApps,
                    isCurrent: captureScope(),
                  })
                }
              >
                {pending ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("platformOnboarding.provisioning")}
                  </>
                ) : (
                  t("platformOnboarding.confirm")
                )}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
