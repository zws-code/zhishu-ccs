import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import type { Provider } from "@/types";
import { platformAuthError } from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import {
  usePlatformSubscriptionUsage,
  type PlatformSubscriptionUsage,
  type PlatformUsageWindow,
} from "@/zhishu/models/query/platformUsage";
import { QuotaLines, useNow } from "@/components/quota/QuotaLines";
import { useQuotaDisplay } from "@/components/quota/useQuotaDisplay";
import {
  balanceLine,
  expiredLine,
  failedLines,
  tierLine,
  type QuotaLine,
} from "@/components/quota/quotaRules";
import {
  tierLabel,
  tierShortLabel,
} from "@/components/SubscriptionQuotaFooter";
import type { QuotaTier } from "@/types/subscription";
import { settingsApi } from "@/lib/api/index";
import { toast } from "@/lib/toast";

interface PlatformUsageFooterProps {
  provider: Provider;
}

function usageTier(name: string, window: PlatformUsageWindow): QuotaTier {
  return {
    name,
    utilization: window.percentage,
    resetsAt: window.resetsAt,
    usedValueUsd: window.usedUsd,
    maxValueUsd: window.limitUsd,
  };
}

function subscriptionDetail(
  subscription: PlatformSubscriptionUsage,
  t: TFunction,
): string {
  return [
    subscription.groupName,
    t("platformUsage.sharedQuota"),
    subscription.expiresInDays > 0
      ? t("platformUsage.expiresInDays", {
          count: subscription.expiresInDays,
        })
      : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

function subscriptionLines(
  subscription: PlatformSubscriptionUsage,
  display: "left" | "used",
  t: TFunction,
): QuotaLine[] {
  const detail = subscriptionDetail(subscription, t);
  const tiers: QuotaTier[] = [];
  if (subscription.daily) tiers.push(usageTier("daily", subscription.daily));
  if (subscription.weekly) {
    tiers.push(usageTier("seven_day", subscription.weekly));
  }
  if (subscription.monthly)
    tiers.push(usageTier("monthly", subscription.monthly));

  if (tiers.length === 0) {
    const text = t("platformUsage.unlimited");
    return [
      {
        key: "unlimited",
        left: Infinity,
        tone: "normal",
        text,
        value: text,
        detail,
      },
    ];
  }

  return tiers.map((tier) => {
    const label = tierLabel(t, tier.name);
    const line = tierLine(t, tier, label, {
      short: tierShortLabel(t, tier.name),
      display,
    });
    return { ...line, detail };
  });
}

/**
 * 平台托管 Provider 的内置额度：订阅分组显示本组共享的日/周/月额度，
 * 当前标准分组显示账号级钱包余额。
 */
export function PlatformUsageFooter({ provider }: PlatformUsageFooterProps) {
  const { t } = useTranslation();
  const auth = usePlatformAuth();
  const display = useQuotaDisplay();
  const query = usePlatformSubscriptionUsage();
  const managed = provider.meta?.platformManaged;
  const now = useNow(Boolean(managed));

  if (!managed || !auth?.account) return null;
  if (
    managed.origin !== auth.account.baseUrl ||
    managed.userId !== auth.account.user.id
  ) {
    return null;
  }

  const groupId = managed.groupId;
  const subscription =
    groupId == null
      ? undefined
      : query.data?.find((item) => item.groupId === groupId);

  if (subscription || managed.subscriptionType === "subscription") {
    const expiredAt = Date.parse(subscription?.expiresAt ?? "");
    const expired =
      subscription?.status === "expired" ||
      (Number.isFinite(expiredAt) && expiredAt <= now);
    let lines: QuotaLine[];
    if (expired) {
      lines = [
        {
          ...expiredLine(t, subscription?.groupName, "platform-expired"),
          detail: subscription?.groupName,
        },
      ];
    } else if (query.isError && !subscription) {
      lines = failedLines(t, platformAuthError(query.error));
    } else if (
      !subscription ||
      (subscription.status && subscription.status !== "active")
    ) {
      lines = [
        {
          key: "platform-subscription-unavailable",
          left: -1,
          tone: query.isFetching ? "muted" : "danger",
          text: t(
            query.isFetching
              ? "platformUsage.subscriptionLoading"
              : "platformUsage.subscriptionInactive",
          ),
        },
      ];
    } else {
      lines = subscriptionLines(subscription, display, t);
    }
    return (
      <div className="flex flex-col items-end gap-1">
        <QuotaLines
          lines={lines}
          queriedAt={query.dataUpdatedAt || null}
          loading={query.isFetching}
          onRefresh={() => query.refetch()}
        />
        <button
          type="button"
          className="text-xs text-fg-3 underline underline-offset-2 hover:text-fg-1"
          onClick={async (event) => {
            event.stopPropagation();
            const website =
              import.meta.env.VITE_PLATFORM_WEB_URL || "http://127.0.0.1:3081";
            try {
              await settingsApi.openExternal(
                `${website.replace(/\/+$/, "")}/subscriptions`,
              );
            } catch {
              toast.error(t("platformUsage.openSubscriptionFailed"));
            }
          }}
        >
          {t(
            expired
              ? "platformUsage.renewSubscription"
              : "platformUsage.manageSubscription",
          )}
        </button>
      </div>
    );
  }

  const balance = auth.account.user.balance;
  const unavailableReason = query.isError
    ? platformAuthError(query.error)
    : null;
  const line = balanceLine(t, {
    key: "platform-account-balance",
    remaining: balance,
    unit: "USD",
    detail: [
      t("platformUsage.accountBalance"),
      t("platformUsage.sharedBalance"),
      unavailableReason
        ? t("platformUsage.subscriptionUnavailable", {
            reason: unavailableReason,
          })
        : null,
    ]
      .filter(Boolean)
      .join(" · "),
  });
  return (
    <QuotaLines
      lines={[line]}
      loading={query.isFetching}
      onRefresh={() => Promise.all([auth.refresh(), query.refetch()])}
    />
  );
}
