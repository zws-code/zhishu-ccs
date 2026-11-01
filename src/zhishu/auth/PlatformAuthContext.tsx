import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  platformAuthApi,
  platformAuthError,
  type PlatformAccount,
  type PlatformLoginResult,
  type PlatformCleanupReport,
} from "@/zhishu/api/platformAuth";

import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/zhishu/api/platformModels";
import { toast } from "@/lib/toast";
import { platformCatalogKeys } from "@/zhishu/models/query/platformCatalogKeys";

interface PlatformAuthContextValue {
  account: PlatformAccount | null;
  loading: boolean;
  logoutAction: "cleanup" | "logout" | null;
  error: string;
  recoveryPending: boolean;
  refresh: () => Promise<void>;
  acceptLogin: (result: PlatformLoginResult) => void;
  logout: () => Promise<void>;
  logoutAndClear: () => Promise<void>;
  cleanupResult: PlatformCleanupReport | null;
  dismissCleanupResult: () => void;
  catalogSyncing: boolean;
  catalogSyncError: string;
  syncCatalog: () => Promise<PlatformCatalog>;
}
const PlatformAuthContext = createContext<PlatformAuthContextValue | null>(
  null,
);

/** 管理平台账户、启动恢复和登录后的目录快照；前端只持有账户摘要，不持有登录令牌。 */
export function PlatformAuthProvider({ children }: { children: ReactNode }) {
  const [account, setAccount] = useState<PlatformAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [logoutAction, setLogoutAction] = useState<"cleanup" | "logout" | null>(
    null,
  );
  const [error, setError] = useState("");
  const [recoveryPending, setRecoveryPending] = useState(false);
  const accountRef = useRef<PlatformAccount | null>(null);
  const queryClient = useQueryClient();
  const refreshing = useRef(false);
  const endingSession = useRef(false);
  const [cleanupResult, setCleanupResult] =
    useState<PlatformCleanupReport | null>(null);
  // 登录/退出/身份变化时递增；异步请求只允许更新自己所属会话，防止迟到响应重建旧缓存。
  const generation = useRef(0);
  const [catalogSyncing, setCatalogSyncing] = useState(false);
  const [catalogSyncError, setCatalogSyncError] = useState("");
  // 同一会话的自动拉取和手动重试复用进行中的目录请求，避免重复请求。
  const catalogRequest = useRef<{
    generation: number;
    promise: Promise<PlatformCatalog>;
  } | null>(null);
  const autoSynced = useRef<string | null>(null);

  const syncCatalog = useCallback(() => {
    const current = generation.current;
    const identity = accountRef.current;
    const changedAccount = () => ({
      code: "UNAUTHORIZED",
      message: "登录状态已改变，请重新同步",
    });
    if (!identity || endingSession.current)
      return Promise.reject(changedAccount());
    if (catalogRequest.current?.generation === current) {
      return catalogRequest.current.promise;
    }
    setCatalogSyncing(true);
    setCatalogSyncError("");
    const promise = (async () => {
      try {
        const result = await platformModelsApi.syncCatalog();
        if (
          current !== generation.current ||
          result.baseUrl !== identity.baseUrl ||
          result.userId !== identity.user.id
        )
          throw changedAccount();
        const queryKey = platformCatalogKeys.account(
          identity.baseUrl,
          identity.user.id,
        );
        await queryClient.cancelQueries({ queryKey });
        if (current !== generation.current) throw changedAccount();
        queryClient.setQueryData(queryKey, result);
        if (current !== generation.current) throw changedAccount();
        return result;
      } catch (cause) {
        if (current === generation.current)
          setCatalogSyncError(platformAuthError(cause));
        throw cause;
      } finally {
        if (current === generation.current) setCatalogSyncing(false);
        if (catalogRequest.current?.generation === current)
          catalogRequest.current = null;
      }
    })();
    catalogRequest.current = { generation: current, promise };
    return promise;
  }, [queryClient]);

  // 每次验证成功的登录或恢复只自动拉取一次平台目录快照（只读），不改写任何工具供应商。
  // 供应商接入由用户在「接入工具」向导中确认后执行；轮询不会重复触发。
  useEffect(() => {
    if (!account) {
      autoSynced.current = null;
      return;
    }
    const identity = JSON.stringify([account.baseUrl, account.user.id]);
    if (autoSynced.current === identity) return;
    autoSynced.current = identity;
    const current = generation.current;
    void (async () => {
      try {
        await syncCatalog();
      } catch (cause) {
        if (current !== generation.current) return;
        toast.error(platformAuthError(cause));
      }
    })();
  }, [account?.baseUrl, account?.user.id, syncCatalog]);

  const refresh = useCallback(async () => {
    if (refreshing.current || endingSession.current) return;
    refreshing.current = true;
    let current = generation.current;
    try {
      const next = await platformAuthApi.account(
        document.visibilityState === "visible",
      );
      if (current === generation.current) {
        const previous = accountRef.current;
        if (
          previous &&
          (!next ||
            previous.baseUrl !== next.baseUrl ||
            previous.user.id !== next.user.id)
        ) {
          current = ++generation.current;
          setCatalogSyncing(false);
          setCatalogSyncError("");
          await queryClient.cancelQueries();
          queryClient.clear();
          if (current !== generation.current) return;
        }
        accountRef.current = next;
        setRecoveryPending(false);
        setAccount(next);
        setError("");
      }
    } catch (cause) {
      if (current === generation.current) {
        setError(platformAuthError(cause));
        // 已验证账户遇到临时故障仍可使用本地功能；启动时尚未验证则进入恢复重试页。
        setRecoveryPending(!accountRef.current);
      }
    } finally {
      refreshing.current = false;
      if (current === generation.current) setLoading(false);
    }
  }, [queryClient]);

  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(() => {
    if (!account && !recoveryPending) return;
    // 后台轮询也可检查失效，但只有可见页面才上报活动，后端还会复核窗口是否最小化。
    const timer = window.setInterval(() => void refresh(), 60_000);
    const onVisible = () => {
      if (document.visibilityState === "visible") void refresh();
    };
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("online", onVisible);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("online", onVisible);
    };
  }, [account, recoveryPending, refresh]);

  const acceptLogin = (result: PlatformLoginResult) => {
    if (result.status === "authenticated" && !endingSession.current) {
      setCleanupResult(null);
      generation.current += 1;
      autoSynced.current = null;
      setCatalogSyncError("");
      accountRef.current = result.account;
      setRecoveryPending(false);
      setLoading(false);
      setAccount(result.account);
      setError("");
    }
  };
  const endSession = async (clearAccount: boolean) => {
    if (endingSession.current) return;
    const identity = accountRef.current;
    if (clearAccount && !identity) return;
    endingSession.current = true;
    setLogoutAction(clearAccount ? "cleanup" : "logout");
    setCleanupResult(null);
    // 先阻断旧请求并清空账户缓存，再由 Rust 执行对应的退出流程。
    const current = ++generation.current;
    accountRef.current = null;
    setCatalogSyncing(false);
    setCatalogSyncError("");
    setRecoveryPending(false);
    setLoading(true);
    setAccount(null);
    setError("");
    try {
      await queryClient.cancelQueries();
      queryClient.clear();
      if (clearAccount && identity) {
        const result = await platformAuthApi.logoutAndClear(
          identity.baseUrl,
          identity.user.id,
        );
        if (current === generation.current) setCleanupResult(result);
      } else {
        await platformAuthApi.logout();
      }
    } catch (cause) {
      if (current === generation.current) {
        setError(
          clearAccount
            ? platformAuthError(cause)
            : `本机已退出登录。${platformAuthError(cause)}`,
        );
        // IPC 拒绝可能发生在身份校验前；不能声称已退出，也不能放行未验证账户。
        if (clearAccount) setRecoveryPending(true);
      }
    } finally {
      endingSession.current = false;
      setLogoutAction(null);
      if (current === generation.current) setLoading(false);
    }
  };
  const logout = () => endSession(false);
  const logoutAndClear = () => endSession(true);

  return (
    <PlatformAuthContext.Provider
      value={{
        account,
        loading,
        logoutAction,
        error,
        recoveryPending,
        refresh,
        acceptLogin,
        logout,
        logoutAndClear,
        cleanupResult,
        dismissCleanupResult: () => setCleanupResult(null),
        catalogSyncing,
        catalogSyncError,
        syncCatalog,
      }}
    >
      {children}
    </PlatformAuthContext.Provider>
  );
}

export const usePlatformAuth = () => useContext(PlatformAuthContext);
