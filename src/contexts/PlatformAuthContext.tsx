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

interface PlatformAuthContextValue {
  account: PlatformAccount | null;
  loading: boolean;
  error: string;
  recoveryPending: boolean;
  refresh: () => Promise<void>;
  acceptLogin: (result: PlatformLoginResult) => void;
  logout: () => Promise<void>;
}
const PlatformAuthContext = createContext<PlatformAuthContextValue | null>(
  null,
);

export function PlatformAuthProvider({ children }: { children: ReactNode }) {
  const [account, setAccount] = useState<PlatformAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [recoveryPending, setRecoveryPending] = useState(false);
  const accountRef = useRef<PlatformAccount | null>(null);
  const queryClient = useQueryClient();
  const refreshing = useRef(false);
  const generation = useRef(0);

  const refresh = useCallback(async () => {
    if (refreshing.current) return;
    refreshing.current = true;
    const current = generation.current;
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
      accountRef.current = result.account;
      setRecoveryPending(false);
      setLoading(false);
      setAccount(result.account);
      setError("");
    }
  };
  const logout = async () => {
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
      }}
    >
      {children}
    </PlatformAuthContext.Provider>
  );
}

export const usePlatformAuth = () => useContext(PlatformAuthContext);
