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
} from "@/zhishu/api/platformAuth";

interface PlatformAuthContextValue {
  account: PlatformAccount | null;
  loading: boolean;
  logoutAction: "logout" | null;
  error: string;
  recoveryPending: boolean;
  refresh: () => Promise<void>;
  acceptLogin: (result: PlatformLoginResult) => void;
  logout: () => Promise<void>;
}
const PlatformAuthContext = createContext<PlatformAuthContextValue | null>(
  null,
);

/** 管理平台账户与启动恢复；前端只持有账户摘要，不持有登录令牌。 */
export function PlatformAuthProvider({ children }: { children: ReactNode }) {
  const [account, setAccount] = useState<PlatformAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [logoutAction, setLogoutAction] = useState<"logout" | null>(null);
  const [error, setError] = useState("");
  const [recoveryPending, setRecoveryPending] = useState(false);
  const accountRef = useRef<PlatformAccount | null>(null);
  const queryClient = useQueryClient();
  const refreshing = useRef(false);
  const endingSession = useRef(false);
  // 登录/退出/身份变化时递增；异步请求只允许更新自己所属会话，防止迟到响应重建旧缓存。
  const generation = useRef(0);

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
      generation.current += 1;
      accountRef.current = result.account;
      setRecoveryPending(false);
      setLoading(false);
      setAccount(result.account);
      setError("");
    }
  };
  const logout = async () => {
    if (endingSession.current) return;
    endingSession.current = true;
    setLogoutAction("logout");
    // 先阻断旧请求并清空账户缓存，再由 Rust 执行退出流程。
    const current = ++generation.current;
    accountRef.current = null;
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
      endingSession.current = false;
      setLogoutAction(null);
      if (current === generation.current) setLoading(false);
    }
  };

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
      }}
    >
      {children}
    </PlatformAuthContext.Provider>
  );
}

export const usePlatformAuth = () => useContext(PlatformAuthContext);
