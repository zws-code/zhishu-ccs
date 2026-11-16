import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { platformAuthApi } from "@/zhishu/api/platformAuth";

/** Public config is scoped to the selected platform; old links disappear on edits. */
export function useClientConfig(baseUrl: string | undefined) {
  const [settled, setSettled] = useState(baseUrl);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(baseUrl), 250);
    return () => clearTimeout(timer);
  }, [baseUrl]);
  const query = useQuery({
    queryKey: ["platformClientConfig", settled],
    queryFn: () => platformAuthApi.config(settled!),
    enabled: Boolean(settled) && settled === baseUrl,
    staleTime: 5 * 60 * 1000,
    retry: false,
  });
  return { ...query, data: settled === baseUrl ? query.data : undefined };
}
