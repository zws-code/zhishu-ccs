import { useCallback, useEffect, useRef } from "react";
import type { PlatformAccount } from "@/zhishu/api/platformAuth";

/** Capture the account and page that started an operation; leaving either invalidates it. */
export function usePlatformOperationScope(
  account: PlatformAccount | null | undefined,
  page: string,
) {
  const identity = account
    ? JSON.stringify([account.baseUrl, account.user.id, page])
    : null;
  const scope = useRef({ identity, active: true });
  if (scope.current.identity !== identity) {
    scope.current = { identity, active: true };
  }
  useEffect(() => {
    const current = scope.current;
    current.active = true;
    return () => {
      current.active = false;
    };
  }, [identity]);

  return useCallback(() => {
    const current = scope.current;
    return () =>
      current.active && current.identity !== null && scope.current === current;
  }, []);
}
