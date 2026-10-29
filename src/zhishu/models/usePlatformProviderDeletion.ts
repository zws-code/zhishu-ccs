import { useEffect, useRef } from "react";
import {
  useIsMutating,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { Provider } from "@/types";
import type { AppId } from "@/lib/api/index";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { PLATFORM_SYNC_APPS } from "@/zhishu/models/platformConfig";
import { platformModelsApi } from "@/zhishu/api/platformModels";
import { platformAuthError } from "@/zhishu/api/platformAuth";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
import { toast } from "@/lib/toast";

/** 挂在页面而非菜单内容中；app 为 null 时删除当前账户在全部已支持应用中的供应商。 */
export function usePlatformProviderDeletion(
  app: AppId | null,
  providers: Record<string, Provider> = {},
) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const auth = usePlatformAuth();
  const account = auth?.account;
  const identity = account
    ? JSON.stringify([account.baseUrl, account.user.id])
    : null;
  const currentIdentity = useRef(identity);
  currentIdentity.current = identity;
  useEffect(() => {
    currentIdentity.current = identity;
    return () => {
      currentIdentity.current = null;
    };
  }, [identity]);
  const count = account
    ? Object.values(providers).filter(
        (provider) =>
          provider.meta?.platformManaged?.origin === account.baseUrl &&
          provider.meta?.platformManaged?.userId === account.user.id,
      ).length
    : 0;
  const providerSyncing = useIsMutating({
    mutationKey: ["platformSyncProviders"],
  });
  const providerDeleting = useIsMutating({
    mutationKey: ["platformDeleteProviders"],
  });
  const deletion = useMutation({
    mutationKey: ["platformDeleteProviders"],
    mutationFn: ({
      app,
      baseUrl,
      userId,
    }: {
      app: AppId | null;
      baseUrl: string;
      userId: number;
      identity: string;
    }) => platformModelsApi.deleteProviders(app, baseUrl, userId),
    onSuccess: async (result, target) => {
      if (currentIdentity.current !== target.identity) return;
      await refreshPlatformProviders(queryClient, target.app ?? undefined);
      if (currentIdentity.current !== target.identity) return;
      const message = t("platformCatalog.deleteCompleted", {
        defaultValue: "已删除 {{removed}} 个平台供应商，保留 {{skipped}} 个",
        removed: result.removed,
        skipped: result.skipped,
      });
      if (result.warnings.length) {
        toast.warning(message, {
          description: result.warnings.join("\n"),
          closeButton: true,
        });
      } else {
        toast.success(message);
      }
    },
    onError: async (error, target) => {
      if (currentIdentity.current !== target.identity) return;
      await refreshPlatformProviders(queryClient, target.app ?? undefined);
      if (currentIdentity.current === target.identity)
        toast.error(platformAuthError(error));
    },
  });
  const visible =
    Boolean(account) && (app === null || PLATFORM_SYNC_APPS.has(app));
  const disabled =
    !visible ||
    (app !== null && !count) ||
    deletion.isPending ||
    providerDeleting > 0 ||
    providerSyncing > 0 ||
    Boolean(auth?.catalogSyncing);
  const matchesTarget =
    deletion.variables?.identity === identity &&
    deletion.variables?.app === app;
  return {
    visible,
    disabled,
    pending: deletion.isPending,
    result: matchesTarget ? deletion.data : undefined,
    error:
      matchesTarget && deletion.isError
        ? platformAuthError(deletion.error)
        : null,
    remove: () => {
      if (disabled || !account || !identity) return;
      deletion.mutate({
        app,
        baseUrl: account.baseUrl,
        userId: account.user.id,
        identity,
      });
    },
  };
}
