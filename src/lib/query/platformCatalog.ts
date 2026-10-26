import { useQuery } from "@tanstack/react-query";
import { usePlatformAuth } from "@/contexts/PlatformAuthContext";
import { platformModelsApi } from "@/lib/api/platformModels";

import { platformCatalogKeys } from "./platformCatalogKeys";
export { platformCatalogKeys } from "./platformCatalogKeys";

/** 读取当前账户的本机目录；只由显式同步更新缓存，不因页面重渲染或时间流逝隐式联网。 */
export function usePlatformCatalog(enabled = true) {
  const account = usePlatformAuth()?.account;
  return useQuery({
    queryKey: platformCatalogKeys.account(
      account?.baseUrl ?? "",
      account?.user.id ?? 0,
    ),
    queryFn: async () => {
      const catalog = await platformModelsApi.catalog();
      return catalog?.baseUrl === account?.baseUrl &&
        catalog?.userId === account?.user.id
        ? catalog
        : null;
    },
    enabled: Boolean(account) && enabled,
    staleTime: Infinity,
    retry: false,
  });
}
