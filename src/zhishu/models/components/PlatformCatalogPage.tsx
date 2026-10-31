import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Database, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AppPageHeader } from "@/components/shell/AppPageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Sheet, SheetTrigger } from "@/components/ui/sheet";
import { PlatformConnectButton } from "@/zhishu/models/components/PlatformConnectButton";
import { PlatformCatalogDetails } from "@/zhishu/models/components/PlatformCatalogDetails";
import { PlatformProviderDeletionDialog } from "@/zhishu/models/components/PlatformProviderDeletionDialog";
import { usePlatformProviderDeletion } from "@/zhishu/models/usePlatformProviderDeletion";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { platformAuthError } from "@/zhishu/api/platformAuth";
import { platformModelsApi } from "@/zhishu/api/platformModels";
import {
  platformCatalogKeys,
  usePlatformCatalog,
} from "@/zhishu/models/query/platformCatalog";
import { toast } from "@/lib/toast";

/** 展示设备快照，提供联网更新基础数据、本地同步及全应用删除供应商入口。 */
export function PlatformCatalogPage() {
  const account = usePlatformAuth()?.account;
  return (
    <AccountCatalogPage
      key={JSON.stringify([account?.baseUrl, account?.user.id])}
    />
  );
}

function AccountCatalogPage() {
  const { t, i18n } = useTranslation();
  const auth = usePlatformAuth();
  const account = auth?.account;
  const queryClient = useQueryClient();
  const catalog = usePlatformCatalog();
  const deletion = usePlatformProviderDeletion(null, undefined, {
    notify: false,
  });
  const sync = useMutation({
    mutationFn: () => (auth?.syncCatalog ?? platformModelsApi.syncCatalog)(),
    onSuccess: async (result) => {
      const queryKey = platformCatalogKeys.account(
        result.baseUrl,
        result.userId,
      );
      // 先取消旧的本地读取，防止它覆盖刚同步的目录。
      await queryClient.cancelQueries({ queryKey });
      // 退出会立即清空缓存；迟到的同步响应不得重新创建已清理的账户查询。
      if (!queryClient.getQueryState(queryKey)) return;
      queryClient.setQueryData(queryKey, result);
      if (
        result.warnings.length ||
        result.groups.some((group) => group.failed)
      ) {
        toast.warning(
          result.warnings[0] ??
            t(
              "platformCatalog.partial",
              "部分分组同步失败，已保留可用数据，可重试",
            ),
        );
      } else {
        toast.success(
          t(
            "platformCatalog.success",
            "平台基础数据已更新，请到各应用同步平台供应商",
          ),
        );
      }
    },
    onError: (error) => toast.error(platformAuthError(error)),
  });
  const data = catalog.data;
  const modelCount =
    data?.groups.reduce((count, group) => count + group.models.length, 0) ?? 0;

  return (
    <>
      <AppPageHeader
        icon={<Database className="h-5 w-5" />}
        title={t("platformCatalog.title", "平台基础数据")}
        actions={
          <>
            <Button
              variant="outline"
              className="text-danger hover:text-danger"
              disabled={deletion.disabled || sync.isPending}
              aria-busy={deletion.pending}
              title={t(
                "platformCatalog.deleteAllHint",
                "删除当前账号在所有 Agent 应用中的平台供应商（含未显示应用），保留平台基础数据和服务端 Key。",
              )}
              onClick={deletion.remove}
            >
              {deletion.pending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Trash2 className="h-4 w-4" />
              )}
              {deletion.pending
                ? t("platformCatalog.deletingProviders", "正在删除平台供应商…")
                : t("platformCatalog.deleteProviders", "一键删除平台供应商")}
            </Button>
            <PlatformConnectButton
              disabled={
                !data ||
                deletion.pending ||
                sync.isPending ||
                auth?.catalogSyncing
              }
            />
            <Button
              disabled={
                !account ||
                sync.isPending ||
                auth?.catalogSyncing ||
                deletion.pending
              }
              aria-busy={sync.isPending || auth?.catalogSyncing}
              onClick={() => sync.mutate()}
            >
              {sync.isPending || auth?.catalogSyncing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              {sync.isPending || auth?.catalogSyncing
                ? t("platformCatalog.syncing", "正在同步平台数据…")
                : t("platformCatalog.sync", "同步平台基础数据")}
            </Button>
          </>
        }
      />
      {account && (deletion.result || deletion.error) && (
        <PlatformProviderDeletionDialog
          account={account}
          result={deletion.result}
          error={deletion.error}
          onDismiss={deletion.dismissResult}
        />
      )}
      <div
        id="main-content"
        className="min-h-0 flex-1 overflow-y-auto px-6 pb-10 pt-4"
      >
        <div className="mb-4 space-y-3 text-body text-fg-2">
          <p className="text-caption">
            {t(
              "platformCatalog.description",
              "从智枢平台获取分组和模型目录，作为所有应用共用的基础数据。更新后，在各应用点击「同步平台供应商」生成本地供应商配置。",
            )}
          </p>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-1 rounded-control bg-subtle px-3 py-2 text-caption">
            <span>
              <span className="me-2 text-fg-3">
                {t("platformCatalog.account", "当前账号")}
              </span>
              <span className="text-fg-1" title={account?.baseUrl}>
                {account?.user.email} · ID {account?.user.id}
              </span>
            </span>
            <span>
              <span className="me-2 text-fg-3">
                {t("platformCatalog.lastSync", "上次同步")}
              </span>
              {data
                ? new Date(data.syncedAt).toLocaleString(
                    i18n.resolvedLanguage ?? i18n.language,
                  )
                : t("platformCatalog.never", "尚未同步")}
            </span>
            {data && (
              <span className="font-medium text-fg-1">
                {t("platformCatalog.counts", {
                  defaultValue: "{{groups}} 个分组 · {{models}} 个模型",
                  groups: data.groups.length,
                  models: modelCount,
                })}
              </span>
            )}
          </div>
          <p className="break-all text-caption text-fg-2">{account?.baseUrl}</p>
          <p className="text-caption text-fg-3">
            {t("platformCatalog.accountIsolation")}
          </p>
        </div>
        {catalog.isLoading && (
          <p role="status">
            {t("platformCatalog.loading", "正在读取本地基础数据…")}
          </p>
        )}
        {catalog.isError && (
          <p role="alert" className="mb-4 text-danger">
            {platformAuthError(catalog.error)}
          </p>
        )}
        {auth?.catalogSyncError && !sync.isError && (
          <p role="alert" className="mb-4 text-danger">
            {auth.catalogSyncError}
          </p>
        )}
        {sync.isError && (
          <p role="alert" className="mb-4 text-danger">
            {platformAuthError(sync.error)}
          </p>
        )}
        {!catalog.isLoading && !data && !catalog.isError && (
          <p className="text-fg-3">
            {t(
              "platformCatalog.empty",
              "尚无当前账号的基础数据，请先点击上方按钮同步。",
            )}
          </p>
        )}
        {data?.warnings.map((warning, index) => (
          <p role="status" key={index} className="mb-2 text-body text-fg-2">
            {warning}
          </p>
        ))}
        {data && data.groups.length === 0 && (
          <p className="text-fg-3">
            {t("platformCatalog.noGroups", "当前账号没有可同步的分组。")}
          </p>
        )}
        <div className="overflow-x-auto rounded-lg border border-border">
          <table
            className="w-full min-w-[640px] table-fixed text-start text-body"
            aria-label={t("platformCatalog.title", "平台基础数据")}
          >
            <thead className="bg-subtle text-caption text-fg-3">
              <tr>
                <th
                  scope="col"
                  className="w-[32%] px-3 py-2 text-start font-medium"
                >
                  {t("platformCatalog.group", "分组")}
                </th>
                <th
                  scope="col"
                  className="w-[16%] px-3 py-2 text-start font-medium"
                >
                  {t("platformCatalog.platform", "平台类型")}
                </th>
                <th
                  scope="col"
                  className="w-[15%] px-3 py-2 text-start font-medium"
                >
                  {t("platformCatalog.billing", "计费类型")}
                </th>
                <th
                  scope="col"
                  className="w-[8%] px-3 py-2 text-end font-medium"
                >
                  {t("platformCatalog.models", "模型")}
                </th>
                <th
                  scope="col"
                  className="w-[16%] px-3 py-2 text-start font-medium"
                >
                  {t("platformCatalog.status", "同步状态")}
                </th>
                <th
                  scope="col"
                  className="w-[13%] px-3 py-2 text-end font-medium"
                >
                  {t("platformCatalog.actions", "操作")}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {data?.groups.map((group) => (
                <Sheet key={`${data.baseUrl}:${data.userId}:${group.id}`}>
                  <tr className="bg-surface hover:bg-subtle">
                    <td className="px-3 py-2">
                      <div className="flex min-w-0 items-center gap-2">
                        <h2
                          className="truncate text-body font-medium"
                          title={group.name}
                        >
                          {group.name}
                        </h2>
                        <span className="shrink-0 font-mono text-caption text-fg-3">
                          #{group.id}
                        </span>
                      </div>
                      {group.error && (
                        <p
                          className="mt-1 truncate text-caption text-danger"
                          title={group.error}
                        >
                          {group.error}
                        </p>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <Badge
                        variant="outline"
                        className="max-w-full rounded-control px-2 py-0 font-mono text-caption font-normal"
                      >
                        <span className="truncate">{group.platform}</span>
                      </Badge>
                    </td>
                    <td className="px-3 py-2 text-caption">
                      {group.subscriptionType === "subscription" ? (
                        <Badge
                          variant="outline"
                          className="rounded-control border-action/30 bg-action/10 px-2 py-0 text-caption text-action-text"
                        >
                          {t("platformCatalog.subscription", "订阅")}
                        </Badge>
                      ) : group.subscriptionType === "standard" ? (
                        t("platformCatalog.standard", "按量计费")
                      ) : (
                        group.subscriptionType ||
                        t("platformCatalog.notReported", "未提供")
                      )}
                    </td>
                    <td className="px-3 py-2 text-end font-mono tabular-nums">
                      {group.models.length}
                    </td>
                    <td className="px-3 py-2">
                      <span
                        className={`inline-flex items-center gap-1.5 whitespace-nowrap text-caption ${group.failed ? "text-danger" : "text-fg-2"}`}
                      >
                        <span
                          aria-hidden="true"
                          className="h-1.5 w-1.5 rounded-full bg-current"
                        />
                        {group.failed
                          ? t("platformCatalog.failed", "同步失败，可重试")
                          : t("platformCatalog.ready", "已同步")}
                      </span>
                    </td>
                    <td className="px-3 py-2 text-end">
                      <SheetTrigger asChild>
                        <Button
                          variant="outline"
                          size="compact"
                          aria-label={t("platformCatalog.viewDetailsFor", {
                            defaultValue: "查看 {{name}} 的详情",
                            name: group.name,
                          })}
                        >
                          {t("platformCatalog.viewDetails", "查看详情")}
                        </Button>
                      </SheetTrigger>
                    </td>
                  </tr>
                  <PlatformCatalogDetails
                    group={group}
                    baseUrl={data.baseUrl}
                    userId={data.userId}
                    syncedAt={data.syncedAt}
                  />
                </Sheet>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </>
  );
}
