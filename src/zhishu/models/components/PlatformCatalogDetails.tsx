import { Database, Gauge, KeyRound, Layers3 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import type { PlatformCatalogGroup } from "@/zhishu/api/platformModels";
import { PlatformCatalogKey } from "./PlatformCatalogKey";
import { PlatformCatalogModels } from "./PlatformCatalogModels";
import { PlatformQuotaStatus } from "./PlatformQuotaStatus";

/** 只读本机快照；切换页签不联网，Key 仍需显式查看或复制。 */
export function PlatformCatalogDetails({
  group,
  baseUrl,
  userId,
  syncedAt,
}: {
  group: PlatformCatalogGroup;
  baseUrl: string;
  userId: number;
  syncedAt: number;
}) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const unknown = t("platformCatalog.notReported", "未提供");
  const billing =
    group.subscriptionType === "subscription"
      ? t("platformCatalog.subscription", "订阅")
      : group.subscriptionType === "standard"
        ? t("platformCatalog.standard", "按量计费")
        : group.subscriptionType || unknown;
  const identity = JSON.stringify([baseUrl, userId, group.id, syncedAt]);
  return (
    <SheetContent
      width={600}
      closeLabel={t("common.close", "关闭")}
      dismissOnOutsideClick
    >
      <SheetHeader className="space-y-3 px-6 pb-5 pt-6">
        <div className="flex items-start gap-3 pe-5">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-control border border-border bg-subtle text-fg-2">
            <Layers3 className="h-5 w-5" />
          </div>
          <div className="min-w-0 space-y-1">
            <SheetTitle className="break-words text-lg font-semibold">
              {group.name}
            </SheetTitle>
            <SheetDescription>
              {t("platformDetails.description")}
            </SheetDescription>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-caption">
          <span className="rounded-control border border-border px-2 py-0.5 font-mono text-fg-2">
            {group.platform || unknown}
          </span>
          <span className="rounded-control bg-subtle px-2 py-0.5 text-fg-2">
            {billing}
          </span>
          <span
            className={`inline-flex items-center gap-1.5 rounded-control px-2 py-0.5 ${group.failed ? "bg-warning-soft text-warning-text" : "bg-success-soft text-success-text"}`}
          >
            <span aria-hidden className="h-1.5 w-1.5 rounded-full bg-current" />
            {group.failed
              ? t("platformCatalog.failed", "同步失败，可重试")
              : t("platformCatalog.ready", "已同步")}
          </span>
        </div>
      </SheetHeader>
      <Tabs
        key={identity}
        defaultValue="overview"
        className="flex min-h-0 flex-1 flex-col"
      >
        <div className="shrink-0 border-b border-border px-6 pb-4">
          <TabsList
            aria-label={t("platformDetails.tabs")}
            className="grid w-full grid-cols-2"
          >
            <TabsTrigger
              value="overview"
              className="min-w-0 data-[state=active]:bg-surface data-[state=active]:text-fg-1"
            >
              {t("platformDetails.overview")}
            </TabsTrigger>
            <TabsTrigger
              value="models"
              className="min-w-0 gap-2 data-[state=active]:bg-surface data-[state=active]:text-fg-1"
            >
              {t("platformCatalog.modelCatalog", "模型目录")}
              <span className="rounded bg-surface px-1.5 text-caption tabular-nums text-fg-3">
                {group.models.length}
              </span>
            </TabsTrigger>
          </TabsList>
        </div>
        <TabsContent
          value="overview"
          className="mt-0 min-h-0 flex-1 overflow-y-auto scroll-stable overscroll-contain"
        >
          <div className="space-y-5 px-6 py-5">
            {group.error && (
              <p
                role="status"
                className="break-words rounded-control bg-warning-soft px-3 py-2.5 text-caption leading-relaxed text-warning-text"
              >
                {group.error}
              </p>
            )}
            <section className="space-y-3">
              <h3 className="flex items-center gap-2 text-body font-medium">
                <Database className="h-4 w-4 text-fg-3" />
                {t("platformDetails.groupInfo")}
              </h3>
              <dl className="grid grid-cols-2 gap-x-4 gap-y-4 rounded-lg border border-border p-4 text-caption">
                {[
                  [t("platformCatalog.groupId", "分组 ID"), String(group.id)],
                  [t("platformCatalog.userId", "用户 ID"), String(userId)],
                  [
                    t("platformCatalog.groupStatus", "分组状态"),
                    group.status || unknown,
                  ],
                  [
                    t("platformCatalog.lastSync", "上次同步"),
                    new Date(syncedAt).toLocaleString(locale),
                  ],
                ].map(([label, value]) => (
                  <div key={label} className="min-w-0 space-y-1">
                    <dt className="text-fg-3">{label}</dt>
                    <dd className="break-words font-medium text-fg-1">
                      {value}
                    </dd>
                  </div>
                ))}
                <div className="col-span-2 min-w-0 space-y-1 border-t border-border pt-3">
                  <dt className="text-fg-3">
                    {t("platformCatalog.source", "平台地址")}
                  </dt>
                  <dd className="break-all font-mono text-fg-2">{baseUrl}</dd>
                </div>
              </dl>
            </section>
            <section className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <h3 className="flex items-center gap-2 text-body font-medium">
                  <Gauge className="h-4 w-4 text-fg-3" />
                  {t("clientQuota.title")}
                </h3>
                <span className="text-caption">
                  <PlatformQuotaStatus quota={group.quota} />
                </span>
              </div>
              {group.subscriptions?.map((subscription) => (
                <div
                  key={subscription.id}
                  className="overflow-hidden rounded-lg border border-border text-caption"
                >
                  <div className="flex flex-wrap items-center justify-between gap-2 bg-subtle px-3 py-2">
                    <span className="font-medium">
                      #{subscription.id} · {subscription.status}
                    </span>
                    <span className="break-all text-fg-3">
                      {t("platformDetails.expires")} {subscription.expires_at}
                    </span>
                  </div>
                  <table className="w-full text-start">
                    <caption className="sr-only">
                      {t("clientQuota.title")} #{subscription.id}
                    </caption>
                    <thead className="text-fg-3">
                      <tr>
                        <th className="px-3 py-2 text-start font-normal">
                          {t("platformDetails.period")}
                        </th>
                        <th className="px-3 py-2 text-end font-normal">
                          {t("platformDetails.used")}
                        </th>
                        <th className="px-3 py-2 text-end font-normal">
                          {t("platformDetails.limit")}
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      {(["daily", "weekly", "monthly"] as const).map(
                        (period) => (
                          <tr key={period}>
                            <th className="px-3 py-2 text-start font-normal text-fg-2">
                              {t(`clientQuota.${period}`)}
                            </th>
                            <td className="px-3 py-2 text-end font-mono tabular-nums">
                              {subscription[period].used_usd.toLocaleString(
                                locale,
                              )}
                            </td>
                            <td className="px-3 py-2 text-end tabular-nums">
                              {subscription[period].limit_usd?.toLocaleString(
                                locale,
                              ) ?? t("clientQuota.noLimit")}
                            </td>
                          </tr>
                        ),
                      )}
                    </tbody>
                  </table>
                </div>
              ))}
            </section>
            <section aria-label="API Key" className="space-y-3">
              <h3 className="flex items-center gap-2 text-body font-medium">
                <KeyRound className="h-4 w-4 text-fg-3" />
                API Key
              </h3>
              <div className="space-y-3 rounded-lg border border-border p-4">
                <dl className="flex flex-wrap gap-x-5 gap-y-2 text-caption">
                  <div className="min-w-0 flex-1 space-y-1">
                    <dt className="text-fg-3">
                      {t("platformCatalog.keyName", "Key 名称")}
                    </dt>
                    <dd className="break-all font-medium">
                      {group.keyName || unknown}
                    </dd>
                  </div>
                  <div className="space-y-1">
                    <dt className="text-fg-3">
                      {t("platformCatalog.keyId", "Key ID")}
                    </dt>
                    <dd className="font-mono">{group.keyId ?? unknown}</dd>
                  </div>
                </dl>
                <PlatformCatalogKey
                  key={identity}
                  baseUrl={baseUrl}
                  userId={userId}
                  groupId={group.id}
                  available={group.keyAvailable !== false}
                />
              </div>
            </section>
          </div>
        </TabsContent>
        <TabsContent
          value="models"
          className="mt-0 flex min-h-0 flex-1 flex-col data-[state=inactive]:hidden"
        >
          <PlatformCatalogModels models={group.models} />
        </TabsContent>
      </Tabs>
    </SheetContent>
  );
}
