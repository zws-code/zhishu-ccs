import { useEffect, useRef } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { Provider } from "@/types";
import type { AppId } from "@/lib/api";
import { usePlatformAuth } from "@/contexts/PlatformAuthContext";
import { PLATFORM_SYNC_APPS } from "@/config/platformConfig";
import { platformModelsApi } from "@/lib/api/platformModels";
import { platformAuthError } from "@/lib/api/platformAuth";
import { refreshPlatformProviders } from "@/lib/query/platformProviders";
import { toast } from "@/lib/toast";

/** 挂在应用页而非菜单内容中，菜单关闭后仍能展示进度、刷新列表。 */
export function usePlatformProviderDeletion(
  app: AppId,
  providers: Record<string, Provider>,
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
  const deletion = useMutation({
    mutationFn: ({
      app,
      baseUrl,
      userId,
    }: {
      app: AppId;
      baseUrl: string;
      userId: number;
      identity: string;
    }) => platformModelsApi.deleteProviders(app, baseUrl, userId),
    onSuccess: async (result, target) => {
      if (currentIdentity.current !== target.identity) return;
      await refreshPlatformProviders(queryClient, target.app);
      if (currentIdentity.current !== target.identity) return;
      const message = t("platformCatalog.deleteCompleted", {
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
      await refreshPlatformProviders(queryClient, target.app);
      if (currentIdentity.current === target.identity)
        toast.error(platformAuthError(error));
    },
  });
  const visible = Boolean(account) && PLATFORM_SYNC_APPS.has(app);
  const disabled =
    !visible || !count || deletion.isPending || Boolean(auth?.catalogSyncing);
  return {
    visible,
    disabled,
    pending: deletion.isPending,
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
