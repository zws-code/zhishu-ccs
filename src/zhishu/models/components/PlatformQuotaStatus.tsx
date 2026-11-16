import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { PlatformQuota } from "@/zhishu/api/platformUsage";

const reasons = new Set([
  "key_missing",
  "key_expired",
  "key_quota_exhausted",
  "key_disabled",
  "key_rate_limit_exhausted",
  "balance_insufficient",
  "subscription_expired",
  "subscription_suspended",
  "subscription_quota_exhausted",
]);
export function quotaText(
  quota: PlatformQuota | null | undefined,
  t: TFunction,
): string {
  if (!quota) return t("platformCatalog.notReported");
  if (quota.available) return t("clientQuota.available");
  return quota.reasons.length
    ? quota.reasons
        .map((reason) =>
          t(`clientQuota.${reasons.has(reason) ? reason : "unavailable"}`),
        )
        .join(" · ")
    : t("clientQuota.unavailable");
}
export function PlatformQuotaStatus({
  quota,
}: {
  quota?: PlatformQuota | null;
}) {
  const { t } = useTranslation();
  return (
    <span className={quota?.available === false ? "text-danger" : "text-fg-2"}>
      {quotaText(quota, t)}
    </span>
  );
}
