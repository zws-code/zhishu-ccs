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
} from "@/lib/api/platformAuth";

import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/lib/api/platformModels";
import { toast } from "@/lib/toast";
import { refreshPlatformProviders } from "@/lib/query/platformProviders";
import { platformCatalogKeys } from "@/lib/query/platformCatalogKeys";

interface PlatformAuthContextValue {
  account: PlatformAccount | null;
  loading: boolean;
  error: string;
  recoveryPending: boolean;
  refresh: () => Promise<void>;
  acceptLogin: (result: PlatformLoginResult) => void;
  logout: () => Promise<void>;
  catalogSyncing: boolean;
  catalogSyncError: string;
  syncCatalog: () => Promise<PlatformCatalog>;
}
const PlatformAuthContext = createContext<PlatformAuthContextValue | null>(
  null,
);

/** 管理平台账户、启动恢复和登录后的两阶段同步；前端只持有账户摘要，不持有登录令牌。 */
export function PlatformAuthProvider({ children }: { children: ReactNode }) {
  const [account, setAccount] = useState<PlatformAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [recoveryPending, setRecoveryPending] = useState(false);
  const accountRef = useRef<PlatformAccount | null>(null);
  const queryClient = useQueryClient();
  const refreshing = useRef(false);
  // 登录/退出/身份变化时递增；异步请求只允许更新自己所属会话，防止迟到响应重建旧缓存。
  const generation = useRef(0);
  const [autoSyncing, setAutoSyncing] = useState(false);
  const [catalogSyncing, setCatalogSyncing] = useState(false);
  const [catalogSyncError, setCatalogSyncError] = useState("");
  // 同一会话的自动同步和手动重试复用进行中的目录请求，避免重复创建或管理分组 Key。
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
    if (!identity) return Promise.reject(changedAccount());
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

  // 每次验证成功的登录或恢复只自动尝试一次：先拉平台目录，再同步全部工具；轮询不重复触发。
  useEffect(() => {
    if (!account) {
      autoSynced.current = null;
      return;
    }
    const identity = JSON.stringify([account.baseUrl, account.user.id]);
    if (autoSynced.current === identity) return;
    autoSynced.current = identity;
    const current = generation.current;
    setAutoSyncing(true);
    void (async () => {
      try {
        await syncCatalog();
        if (current !== generation.current) return;
        const result = await platformModelsApi.sync();
        if (current !== generation.current) return;
        await refreshPlatformProviders(queryClient);
        if (current !== generation.current) return;
        if (result.warnings.length) {
          setCatalogSyncError(result.warnings.join("\n"));
          toast.warning(result.warnings[0]);
        }
      } catch (cause) {
        if (current !== generation.current) return;
        setCatalogSyncError(platformAuthError(cause));
        toast.error(platformAuthError(cause));
        void queryClient.invalidateQueries({
          queryKey: ["configChangeHistory"],
        });
      } finally {
        if (current === generation.current) setAutoSyncing(false);
      }
    })();
  }, [account?.baseUrl, account?.user.id, syncCatalog, queryClient]);

  const refresh = useCallback(async () => {
    if (refreshing.current) return;
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
          setAutoSyncing(false);
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
    if (result.status === "authenticated") {
      generation.current += 1;
      autoSynced.current = null;
      setAutoSyncing(false);
      setCatalogSyncError("");
      accountRef.current = result.account;
      setRecoveryPending(false);
      setLoading(false);
      setAccount(result.account);
      setError("");
    }
  };
  const logout = async () => {
    // 先阻断前端旧请求并清空账户缓存，再让 Rust 清理登录会话；本地 API Key 保留。
    const current = ++generation.current;
    accountRef.current = null;
    setAutoSyncing(false);
    setCatalogSyncing(false);
    setCatalogSyncError("");
    setRecoveryPending(false);
    setLoading(true);
    setAccount(null);
    setError("");
    try {
      await queryClient.cancelQueries();
      queryClient.clear();
      await platformAuthApi.logout();
    } catch (cause) {
      if (current === generation.current) {
        setError(`本机已退出登录。${platformAuthError(cause)}`);
      }
    } finally {
      if (current === generation.current) setLoading(false);
    }
  };

  return (
    <PlatformAuthContext.Provider
      value={{
        account,
        loading,
        error,
        recoveryPending,
        refresh,
        acceptLogin,
        logout,
        catalogSyncing: catalogSyncing || autoSyncing,
        catalogSyncError,
        syncCatalog,
      }}
    >
      {children}
    </PlatformAuthContext.Provider>
  );
}

export const usePlatformAuth = () => useContext(PlatformAuthContext);
