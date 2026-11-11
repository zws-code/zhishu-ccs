import { useQuery } from "@tanstack/react-query";
import type { TFunction } from "i18next";
import type { AppId } from "@/lib/api";
import { settingsApi } from "@/lib/api/index";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import type { AppMode, AppModeView } from "@/types/proxy";
import type { Provider } from "@/types";
import {
  platformModelsApi,
  type PlatformGroupOption,
} from "@/zhishu/api/platformModels";
import {
  platformAuthError,
  type PlatformAccount,
} from "@/zhishu/api/platformAuth";
import { isAdditiveAppId, isStackAppId } from "@/config/appConfig";
import { extractErrorMessage } from "@/utils/errorUtils";
import {
  TOOL_APP_IDS,
  type ToolName,
} from "@/components/apps/useToolManagement";

const TOOL_BY_APP = Object.fromEntries(
  Object.entries(TOOL_APP_IDS).map(([tool, app]) => [app, tool]),
) as Partial<Record<AppId, ToolName>>;

/** 行内分组选择所需的一次性只读数据：可用分组、托管行→分组映射、当前生效托管分组。 */
export interface PlatformGroupSelectionData {
  options: PlatformGroupOption[];
  providerGroups: Record<string, number>;
  currentGroupId: number | null;
}

/** 只读拉取某个应用的分组选择数据；不发平台请求，只读本机目录快照与供应商行。 */
export async function fetchPlatformGroupData(
  app: AppId,
  account: PlatformAccount,
): Promise<PlatformGroupSelectionData> {
  const [options, all, currentId] = await Promise.all([
    platformModelsApi.groupOptions(app),
    providersApi.getAll(app).catch((): Record<string, Provider> => ({})),
    providersApi.getCurrent(app).catch(() => ""),
  ]);
  const providerGroups: Record<string, number> = {};
  for (const provider of Object.values(all)) {
    const managed = provider.meta?.platformManaged;
    if (
      managed?.origin === account.baseUrl &&
      managed.userId === account.user.id &&
      managed.groupId !== undefined
    ) {
      providerGroups[provider.id] = managed.groupId;
    }
  }
  const currentManaged = all[currentId]?.meta?.platformManaged;
  const currentGroupId =
    currentManaged?.origin === account.baseUrl &&
    currentManaged.userId === account.user.id
      ? (currentManaged.groupId ?? null)
      : null;
  return { options, providerGroups, currentGroupId };
}

/**
 * 行内分组默认值：用户选择 → 当前生效直连/路由供应商所属分组 → 当前托管分组 →
 * 目录首项。模式应用优先沿用当前生效的直连/路由供应商所属分组；累加式应用用
 * 当前默认项。
 */
export function pickPlatformGroupId(
  chosen: number | undefined,
  data: PlatformGroupSelectionData | undefined,
  modeData: AppModeView | undefined,
): number | undefined {
  const options = data?.options ?? [];
  if (chosen && options.some((option) => option.groupId === chosen)) {
    return chosen;
  }
  const effectiveProviderId = modeData
    ? modeData.mode === "direct"
      ? modeData.directProviderId
      : modeData.routeProviderId
    : null;
  const current =
    (effectiveProviderId
      ? data?.providerGroups?.[effectiveProviderId]
      : undefined) ?? data?.currentGroupId;
  if (current && options.some((option) => option.groupId === current)) {
    return current;
  }
  return options[0]?.groupId;
}

/** 检测各应用本机是否安装（工具安装报告 + Claude Desktop / WorkBuddy 特判）；向导与应用开关页共用。 */
export function usePlatformAppInstalls(apps: AppId[]) {
  return useQuery({
    queryKey: ["platform-app-installs", apps.join(",")],
    queryFn: async () => {
      const tools = apps
        .map((app) => TOOL_BY_APP[app])
        .filter((tool): tool is ToolName => Boolean(tool));
      const [reports, desktop, workbuddyInstalled] = await Promise.all([
        tools.length
          ? settingsApi.listToolInstallations(tools)
          : Promise.resolve([]),
        apps.includes("claude-desktop")
          ? providersApi.getClaudeDesktopStatus().catch(() => null)
          : Promise.resolve(null),
        apps.includes("workbuddy")
          ? providersApi.getWorkBuddyInstalled().catch(() => false)
          : Promise.resolve(false),
      ]);
      const installed = new Set<AppId>();
      for (const report of reports) {
        if (!report.installs.length) continue;
        const app = TOOL_APP_IDS[report.tool as ToolName];
        if (app) installed.add(app);
      }
      if (desktop?.installed) installed.add("claude-desktop");
      if (workbuddyInstalled) installed.add("workbuddy");
      return installed;
    },
    enabled: apps.length > 0,
    staleTime: 60_000,
    retry: false,
  });
}

export type PlatformProvisionAction = "direct" | "route" | "stack" | "additive";

/** 单个应用的接入结果；`action` 缺省表示失败或未知。 */
export interface PlatformProvisionOutcome {
  app: AppId;
  ok: boolean;
  /** 模式应用的实际接入方式；累加式应用为 additive。 */
  action?: PlatformProvisionAction;
  providerName?: string;
  /** 聚合/累加式应用实际加入的平台供应商数量。 */
  count?: number;
  message?: string;
}

export interface ProvisionAppContext {
  t: TFunction;
  account: PlatformAccount;
  /** 目录分组 id → 目录顺序，托管供应商按它稳定排序。 */
  groupOrder: Map<number, number>;
  /** 行内选择的分组；缺省时按当前生效托管分组/目录首项取。 */
  groupId?: number;
  /** 行内选择的接入方式。 */
  mode: AppMode;
  /** 发起操作时捕获的身份校验；账号切换或页面卸载后返回 false。 */
  isCurrent: () => boolean;
}

/**
 * 对单个应用执行「接入到工具」的接入：按本机目录生成平台供应商，再按所选方式
 * 应用（直连 / 路由 / 聚合 / 累加）。「接入到工具」向导与应用开关页共用这一套
 * 逻辑；中途账号切换或页面卸载（isCurrent 为 false）返回 null 表示放弃。
 */
export async function provisionPlatformApp(
  app: AppId,
  ctx: ProvisionAppContext,
): Promise<PlatformProvisionOutcome | null> {
  const {
    t,
    account,
    groupOrder,
    groupId: chosenGroupId,
    mode: requested,
    isCurrent,
  } = ctx;
  try {
    const summary = await platformModelsApi.sync(app);
    const [all, currentId] = await Promise.all([
      providersApi.getAll(app),
      providersApi.getCurrent(app).catch(() => ""),
    ]);
    const managed = Object.values(all)
      .filter(
        (provider) =>
          provider.meta?.platformManaged?.origin === account.baseUrl &&
          provider.meta?.platformManaged?.userId === account.user.id,
      )
      .sort(
        (left, right) =>
          (groupOrder.get(left.meta?.platformManaged?.groupId ?? -1) ??
            Number.MAX_SAFE_INTEGER) -
          (groupOrder.get(right.meta?.platformManaged?.groupId ?? -1) ??
            Number.MAX_SAFE_INTEGER),
      );
    if (managed.length === 0) {
      return {
        app,
        ok: false,
        message: summary.warnings[0] ?? t("platformOnboarding.noProvider"),
      };
    }
    const syncWarning = summary.warnings[0];

    if (isAdditiveAppId(app)) {
      // 累加式应用没有模式：把该应用的全部平台供应商加入工具列表。
      let added = 0;
      const notices: string[] = [];
      const errors: string[] = [];
      for (const provider of managed) {
        if (!isCurrent()) return null;
        try {
          if (app === "kimi") {
            await providersApi.addToLiveConfig(provider.id, app);
          } else {
            const switched = await providersApi.switch(provider.id, app);
            if (switched.warnings[0]) notices.push(switched.warnings[0]);
          }
          added += 1;
        } catch (cause) {
          errors.push(extractErrorMessage(cause) || platformAuthError(cause));
        }
      }
      const defaultModelHint = ["opencode", "openclaw", "pi", "mcode"].includes(
        app,
      )
        ? t("platformOnboarding.defaultModelHint")
        : undefined;
      return {
        app,
        ok: added > 0,
        action: "additive",
        count: added,
        message:
          [...notices, ...errors, syncWarning, defaultModelHint]
            .filter(Boolean)
            .join("；") || undefined,
      };
    }

    // 使用用户选择的分组；没有选或该分组已不可用时，沿用当前托管分组/目录首项。
    const preferred =
      (chosenGroupId !== undefined
        ? managed.find(
            (provider) =>
              provider.meta?.platformManaged?.groupId === chosenGroupId,
          )
        : undefined) ??
      managed.find((provider) => provider.id === currentId) ??
      managed[0];

    const mode =
      requested === "stack" && !isStackAppId(app) ? "route" : requested;

    if (mode === "direct") {
      // 已经在路由/聚合时先退回直连，再写直连关键字段。
      await proxyApi.setProxyTakeoverForApp(app, false);
      if (!isCurrent()) return null;
      const switched = await providersApi.switch(preferred.id, app);
      return {
        app,
        ok: true,
        action: "direct",
        providerName: preferred.name,
        message:
          [switched.warnings[0], syncWarning].filter(Boolean).join("；") ||
          undefined,
      };
    }

    if (mode === "route") {
      await proxyApi.setProxyTakeoverForApp(app, true, false, preferred.id);
      return {
        app,
        ok: true,
        action: "route",
        providerName: preferred.name,
        message: syncWarning,
      };
    }

    // 聚合：进入聚合、把默认那家设为平台供应商，再把其余平台分组加入名单。
    await proxyApi.setProxyTakeoverForApp(app, true, true, preferred.id);
    if (!isCurrent()) return null;
    let count = 0;
    const stackNotices: string[] = [];
    const stackErrors: string[] = [];
    for (const provider of managed) {
      if (!isCurrent()) return null;
      try {
        const notice = await proxyApi.setProxyStackMember(
          app,
          provider.id,
          true,
        );
        if (notice) stackNotices.push(notice);
        count += 1;
      } catch (cause) {
        stackErrors.push(
          extractErrorMessage(cause) || platformAuthError(cause),
        );
      }
    }
    return {
      app,
      ok: count > 0,
      action: "stack",
      providerName: preferred.name,
      count,
      message:
        [...stackNotices, ...stackErrors, syncWarning]
          .filter(Boolean)
          .join("；") || undefined,
    };
  } catch (cause) {
    return {
      app,
      ok: false,
      message: extractErrorMessage(cause) || platformAuthError(cause),
    };
  }
}
