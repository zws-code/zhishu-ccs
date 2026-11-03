import { useQuery, useQueryClient } from "@tanstack/react-query";
import { configHistoryApi } from "@/zhishu/config-history/api/configHistory";
import type { AppId } from "@/lib/api/types";
import { useTauriEvent } from "@/hooks/useTauriEvent";

export const configHistoryKeys = {
  all: ["configChangeHistory"] as const,
  list: (app?: AppId) => ["configChangeHistory", app ?? "all"] as const,
};

/** 读取按应用筛选的本机配置修改记录，写入事件和相关 mutation 负责使缓存失效。 */
export function useConfigChangeHistory(app?: AppId) {
  const client = useQueryClient();
  useTauriEvent<string>("config-change-history-updated", (changedApp) => {
    if (!changedApp || !app || changedApp === app) {
      void client.invalidateQueries({ queryKey: configHistoryKeys.all });
    }
  });
  return useQuery({
    queryKey: configHistoryKeys.list(app),
    queryFn: () => configHistoryApi.list(app),
    staleTime: 0,
  });
}
