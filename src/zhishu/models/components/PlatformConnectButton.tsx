import {
  useIsMutating,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppId } from "@/lib/api/index";
import { platformModelsApi } from "@/zhishu/api/platformModels";
import { platformAuthError } from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { usePlatformOperationScope } from "@/zhishu/auth/usePlatformOperationScope";
import { Button } from "@/components/ui/button";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
import { toast } from "@/lib/toast";
import { PLATFORM_SYNC_APPS } from "@/zhishu/models/platformConfig";

/** 使用本机目录同步供应商；展示部分成功警告，并分别报告供应商行与原生文件的变化。 */
export function PlatformConnectButton({
  app,
  disabled = false,
}: {
  app?: AppId;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const auth = usePlatformAuth();
  const captureScope = usePlatformOperationScope(auth?.account, app ?? "all");
  const queryClient = useQueryClient();
  const deleting = useIsMutating({ mutationKey: ["platformDeleteProviders"] });
  const sync = useMutation({
    mutationKey: ["platformSyncProviders"],
    mutationFn: ({
      agent,
    }: {
      agent: AppId | undefined;
      isCurrent: () => boolean;
    }) => platformModelsApi.sync(agent),
    onSuccess: async (result, { agent, isCurrent }) => {
      if (!isCurrent()) return;
      await refreshPlatformProviders(queryClient, agent);
      if (!isCurrent()) return;
      const reportMessage =
        result.updated !== undefined && result.nativeFilesChanged !== undefined
          ? t("platformCatalog.syncCompleted", {
              defaultValue:
                "供应商列表：新增 {{added}}，更新 {{updated}}，删除 {{removed}}；{{fileState}}。详情见配置修改记录。",
              ...result,
              fileState:
                result.nativeFilesChanged === 0
                  ? t(
                      "platformCatalog.noNativeChangesShort",
                      "未修改原生配置文件",
                    )
                  : t("platformCatalog.nativeChangedCount", {
                      defaultValue: "{{count}} 个原生配置文件变更",
                      count: result.nativeFilesChanged,
                    }),
            })
          : null;
      if (result.warnings.length > 0) {
        const changeText =
          reportMessage ??
          t("platformCatalog.changes", {
            defaultValue: "新增 {{added}}，删除 {{removed}}，跳过 {{skipped}}",
            ...result,
          });
        toast.warning(`${result.warnings[0]}（${changeText}）`);
      } else {
        toast.success(
          reportMessage ??
            t("platformCatalog.providersSuccess", {
              defaultValue:
                "服务提供商列表已同步：新增 {{added}}，删除 {{removed}}，跳过 {{skipped}}",
              ...result,
            }),
        );
      }
    },
    onError: (error, { isCurrent }) => {
      if (!isCurrent()) return;
      void queryClient.invalidateQueries({ queryKey: ["configChangeHistory"] });
      toast.error(platformAuthError(error));
    },
  });

  if (!auth?.account || (app && !PLATFORM_SYNC_APPS.has(app))) return null;

  return (
    <Button
      variant="outline"
      size="regular"
      disabled={disabled || sync.isPending || deleting > 0}
      aria-busy={sync.isPending}
      title={t(
        "platformCatalog.localHint",
        "使用已同步到本机的平台基础数据生成供应商配置",
      )}
      onClick={() => sync.mutate({ agent: app, isCurrent: captureScope() })}
    >
      {sync.isPending ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <RefreshCw className="h-4 w-4" />
      )}
      {sync.isPending
        ? t("platformCatalog.providersSyncing", "正在同步平台供应商…")
        : app
          ? t("platformCatalog.providersSync", "同步平台供应商")
          : t("platformCatalog.providersSyncAll", "全部同步平台供应商")}
    </Button>
  );
}
