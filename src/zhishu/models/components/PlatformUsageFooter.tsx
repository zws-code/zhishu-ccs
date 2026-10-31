import { useTranslation } from "react-i18next";
import type { QuotaTier } from "@/types/subscription";
import type { Provider } from "@/types";
import { platformAuthError } from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { useClientConfig } from "@/zhishu/auth/useClientConfig";
import { usePlatformSubscriptionUsage } from "@/zhishu/models/query/platformUsage";
import { QuotaLines, useNow } from "@/components/quota/QuotaLines";
import { useQuotaDisplay } from "@/components/quota/useQuotaDisplay";
import {
  balanceLine,
  failedLines,
  tierLine,
  type QuotaLine,
} from "@/components/quota/quotaRules";
import {
  tierLabel,
  tierShortLabel,
} from "@/components/SubscriptionQuotaFooter";
import { settingsApi } from "@/lib/api/index";
import { toast } from "@/lib/toast";
import { quotaText } from "./PlatformQuotaStatus";

export function PlatformUsageFooter({ provider }: { provider: Provider }) {
  const { t } = useTranslation();
  const auth = usePlatformAuth();
  const display = useQuotaDisplay();
  const query = usePlatformSubscriptionUsage();
  const config = useClientConfig(auth?.account?.baseUrl);
  const managed = provider.meta?.platformManaged;
  const now = useNow(Boolean(managed));
  if (
    !managed ||
    !auth?.account ||
    managed.origin !== auth.account.baseUrl ||
    managed.userId !== auth.account.user.id
  )
    return null;
  const summary =
    query.data?.baseUrl === managed.origin &&
    query.data?.userId === managed.userId
      ? query.data
      : undefined;
  const group = summary?.groups.find((group) => group.id === managed.groupId);
  const subscriptionGroup =
    group?.subscription_type === "subscription" ||
    Boolean(group?.subscriptions.length) ||
    managed.subscriptionType === "subscription";
  const expired = group?.quota.reasons.includes("subscription_expired");
  let lines: QuotaLine[] = [];
  if (query.isError) {
    lines = failedLines(t, platformAuthError(query.error));
  } else if (!group) {
    lines = [
      {
        key: "unavailable",
        left: -1,
        tone: query.isFetching ? "muted" : "danger",
        text: t(
          query.isFetching
            ? "platformUsage.subscriptionLoading"
            : "clientQuota.unavailable",
        ),
      },
    ];
  } else if (!group.quota.available) {
    lines = [
      {
        key: "unavailable",
        left: 0,
        tone: "danger",
        text: quotaText(group.quota, t),
        detail: group.name,
      },
    ];
  } else if (subscriptionGroup) {
    const active = group.subscriptions.filter(
      (sub) =>
        sub.active &&
        sub.status === "active" &&
        Date.parse(sub.expires_at) > now,
    );
    for (const sub of active) {
      const detail = `${group.name} · ${t("platformUsage.sharedQuota")} · #${sub.id}`;
      let limited = false;
      for (const [field, tier] of [
        ["daily", "daily"],
        ["weekly", "seven_day"],
        ["monthly", "monthly"],
      ] as const) {
        const window = sub[field];
        if (window.limit_usd == null) continue;
        limited = true;
        const quotaTier: QuotaTier = {
          name: tier,
          utilization:
            window.limit_usd > 0
              ? (window.used_usd / window.limit_usd) * 100
              : 100,
          resetsAt: window.resets_at,
          usedValueUsd: window.used_usd,
          maxValueUsd: window.limit_usd,
        };
        const line = tierLine(t, quotaTier, tierLabel(t, tier), {
          short: tierShortLabel(t, tier),
          display,
        });
        lines.push({ ...line, key: `${sub.id}-${tier}`, detail });
      }
      if (!limited)
        lines.push({
          key: `${sub.id}-unlimited`,
          left: Infinity,
          tone: "normal",
          text: t("platformUsage.unlimited"),
          detail,
        });
    }
    if (!lines.length)
      lines = [
        {
          key: "inactive",
          left: -1,
          tone: "danger",
          text: t("platformUsage.subscriptionInactive"),
        },
      ];
  } else {
    lines = [
      balanceLine(t, {
        key: "balance",
        remaining: summary!.balance,
        unit: "USD",
        detail: `${t("platformUsage.accountBalance")} · ${t("platformUsage.sharedBalance")}`,
      }),
    ];
  }
  return (
    <div className="flex flex-col items-end gap-1">
      <QuotaLines
        lines={lines}
        queriedAt={query.dataUpdatedAt || null}
        loading={query.isFetching}
        onRefresh={() => query.refetch()}
      />
      {subscriptionGroup && config.data?.subscriptions_url && (
        <button
          type="button"
          className="text-xs text-fg-3 underline underline-offset-2 hover:text-fg-1"
          onClick={async (event) => {
            event.stopPropagation();
            try {
              await settingsApi.openExternal(config.data!.subscriptions_url!);
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
      )}
    </div>
  );
}
