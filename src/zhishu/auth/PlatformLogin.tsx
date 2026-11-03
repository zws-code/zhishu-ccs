import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  ChevronDown,
  Loader2,
  LogOut,
  RefreshCw,
  Server,
  ShieldCheck,
  UserRound,
  Wallet,
} from "lucide-react";
import { toast } from "sonner";
import { platformAuthApi, platformAuthError } from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DRAG_REGION_ATTR, DRAG_REGION_STYLE, isMac } from "@/lib/platform";
import { WindowControls } from "@/components/shell/WindowControls";
import { useClientConfig } from "@/zhishu/auth/useClientConfig";
import {
  PlatformAccountCleanupButton,
  PlatformAccountCleanupResult,
} from "./PlatformAccountCleanup";
import logoMark from "@/zhishu/assets/logo-mark.png";

// Vite 根目录是 src/，可选的部署环境配置相应放在 src/.env.local。
const DEFAULT_API =
  import.meta.env.VITE_PLATFORM_API_URL || "http://127.0.0.1:3000";

/** 登录门禁共用的氛围背景：柔光加淡点阵，避免整屏纯色留白显得空。 */
function LoginBackdrop() {
  return (
    <div
      aria-hidden
      className="pointer-events-none fixed inset-0 overflow-hidden"
    >
      <div
        className="absolute left-1/2 top-[-260px] h-[560px] w-[820px] -translate-x-1/2 rounded-full opacity-90 blur-[110px]"
        style={{
          background:
            "radial-gradient(circle, var(--action-soft) 0%, transparent 70%)",
        }}
      />
      <div
        className="absolute bottom-[-220px] left-[-160px] h-[440px] w-[440px] rounded-full opacity-80 blur-[120px]"
        style={{
          background:
            "radial-gradient(circle, var(--direct-soft) 0%, transparent 70%)",
        }}
      />
      <div
        className="absolute inset-0 opacity-60 dark:opacity-30"
        style={{
          backgroundImage:
            "radial-gradient(circle at 1px 1px, hsl(var(--border)) 1px, transparent 0)",
          backgroundSize: "22px 22px",
          maskImage:
            "radial-gradient(ellipse at 50% 42%, black 0%, transparent 72%)",
          WebkitMaskImage:
            "radial-gradient(ellipse at 50% 42%, black 0%, transparent 72%)",
        }}
      />
    </div>
  );
}

/** 登录失败、恢复失败的统一错误条：保留 role="alert" 供读屏与回归用例识别。 */
function LoginError({ message }: { message: string }) {
  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-control bg-danger-soft px-3 py-2 text-caption text-danger-text"
    >
      <AlertCircle className="mt-px h-3.5 w-3.5 shrink-0" />
      <span className="min-w-0 break-words">{message}</span>
    </div>
  );
}

/** 登录页门禁：恢复期间等待验证，恢复失败提供重试入口，通过认证后才挂载工具界面。 */
export function PlatformAuthGate({ children }: { children: ReactNode }) {
  const auth = usePlatformAuth();
  const { t } = useTranslation();
  if (!auth) throw new Error("PlatformAuthProvider is required");
  if (auth.loading)
    return (
      <div
        className="flex h-screen items-center justify-center bg-app text-fg-2"
        role="status"
      >
        <div className="flex flex-col items-center gap-3">
          <Loader2 className="h-5 w-5 animate-spin" />
          <span className="text-body">
            {t(
              auth.logoutAction === "cleanup"
                ? "platformCleanup.working"
                : auth.logoutAction === "logout"
                  ? "platformCleanup.signingOut"
                  : "platformAuth.loading",
            )}
          </span>
        </div>
      </div>
    );
  if (auth.recoveryPending) return <PlatformSessionRecovery />;
  return (
    <>
      {auth.account ? children : <PlatformLogin />}
      {auth.cleanupResult && (
        <PlatformAccountCleanupResult
          result={auth.cleanupResult}
          onDismiss={auth.dismissCleanupResult}
        />
      )}
    </>
  );
}

/** 启动恢复失败页；重试保留现有会话资料，返回登录则先显式退出，避免绕过身份验证。 */
function PlatformSessionRecovery() {
  const auth = usePlatformAuth()!;
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const retry = async () => {
    setBusy(true);
    try {
      await auth.refresh();
    } finally {
      setBusy(false);
    }
  };
  const signIn = async () => {
    setBusy(true);
    try {
      await auth.logout();
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="relative flex h-screen flex-col overflow-y-auto bg-app text-fg-1">
      <LoginBackdrop />
      <div
        className="relative z-10 flex h-11 shrink-0 items-center justify-end px-3"
        {...DRAG_REGION_ATTR}
        style={DRAG_REGION_STYLE}
      >
        {!isMac() && (
          <div className="flex gap-1">
            <WindowControls />
          </div>
        )}
      </div>
      <main className="relative z-10 flex flex-1 flex-col px-6 py-6">
        <div className="m-auto w-full max-w-[400px]">
          <div className="rounded-dialog border border-border bg-surface p-6 shadow-v7-lg">
            <div className="flex h-11 w-11 items-center justify-center rounded-[12px] border border-border bg-app text-fg-2 shadow-v7-sm">
              <RefreshCw className="h-5 w-5" />
            </div>
            <h1 className="mt-4 text-page font-semibold tracking-tight">
              {t("platformAuth.restoreFailed")}
            </h1>
            <p className="mt-1 text-body text-fg-2">
              {t("platformAuth.restoreHint")}
            </p>
            <div className="mt-4">
              <LoginError message={auth.error ?? ""} />
            </div>
            <div className="mt-5 flex gap-2">
              <Button
                className="h-9 flex-1"
                disabled={busy}
                onClick={() => void retry()}
              >
                {busy && <Loader2 className="me-2 h-4 w-4 animate-spin" />}
                {t("platformAuth.retryRestore")}
              </Button>
              <Button
                variant="neutral"
                className="h-9 flex-1"
                disabled={busy}
                onClick={() => void signIn()}
              >
                {t("platformAuth.back")}
              </Button>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

/** 平台登录仅通过浏览器 OAuth2 授权。 */
export function PlatformLogin() {
  const auth = usePlatformAuth();
  const { t } = useTranslation();
  const [baseUrl, setBaseUrl] = useState(DEFAULT_API);
  const config = useClientConfig(baseUrl);
  const [error, setError] = useState("");
  const [oauthBusy, setOauthBusy] = useState(false);
  const oauthCancelled = useRef(false);
  const oauthAvailable = Boolean(
    config.data?.login_methods?.includes("oauth") &&
      config.data?.oauth_authorize_url &&
      config.data?.oauth_client_id,
  );
  if (!auth) return null;

  const startOAuth = async () => {
    if (oauthBusy || !oauthAvailable || config.isFetching) return;
    oauthCancelled.current = false;
    setOauthBusy(true);
    setError("");
    try {
      const result = await platformAuthApi.loginOAuth(baseUrl);
      if (oauthCancelled.current) return;
      if (result.persistenceWarning) toast.warning(result.persistenceWarning);
      auth.acceptLogin(result);
    } catch (cause) {
      if (!oauthCancelled.current) setError(platformAuthError(cause));
    } finally {
      setOauthBusy(false);
    }
  };
  const cancelOAuth = async () => {
    oauthCancelled.current = true;
    setError("");
    try {
      await platformAuthApi.cancelLogin();
    } catch (cause) {
      oauthCancelled.current = false;
      setError(platformAuthError(cause));
    }
  };

  let serverLabel = baseUrl;
  try {
    serverLabel = new URL(baseUrl).host;
  } catch {
    // 地址输入到一半时保留原始文本，不打断编辑。
  }

  return (
    <div className="relative flex h-screen flex-col overflow-y-auto bg-app text-fg-1">
      <LoginBackdrop />
      <div
        className="relative z-10 flex h-11 shrink-0 items-center justify-end px-3"
        {...DRAG_REGION_ATTR}
        style={DRAG_REGION_STYLE}
      >
        {!isMac() && (
          <div className="flex gap-1">
            <WindowControls />
          </div>
        )}
      </div>
      <main className="relative z-10 flex flex-1 flex-col px-6 py-6">
        <div className="m-auto w-full max-w-[400px]">
          <div className="relative overflow-hidden rounded-dialog border border-border bg-surface p-6 shadow-v7-lg">
            <div
              aria-hidden
              className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-action to-transparent opacity-40"
            />
            <header className="flex flex-col items-center text-center">
              <div className="flex h-11 w-11 items-center justify-center rounded-[12px] border border-border bg-app shadow-v7-sm">
                <img
                  src={logoMark}
                  alt=""
                  draggable={false}
                  className="h-[18px] w-auto dark:invert"
                />
              </div>
              <h1 className="mt-4 text-page font-semibold tracking-tight">
                {config.data?.site_name || t("platformAuth.title")}
              </h1>
              <p className="mt-1 text-body text-fg-2">
                {t("platformAuth.subtitle")}
              </p>
            </header>
            <div className="mt-6 space-y-4">
              <Button
                type="button"
                disabled={
                  oauthBusy ||
                  config.isPending ||
                  config.isFetching ||
                  !oauthAvailable
                }
                className="h-9 w-full"
                onClick={() => void startOAuth()}
              >
                {oauthBusy || config.isPending ? (
                  <Loader2 className="me-2 h-4 w-4 animate-spin" />
                ) : (
                  <ShieldCheck className="me-2 h-4 w-4" />
                )}
                {t(
                  oauthBusy
                    ? "platformAuth.oauthWaiting"
                    : "platformAuth.oauthLogin",
                )}
              </Button>
              <p className="text-center text-caption text-fg-3">
                {t("platformAuth.oauthHint")}
              </p>
              {oauthBusy && (
                <Button
                  type="button"
                  variant="ghost"
                  size="compact"
                  className="w-full"
                  onClick={() => void cancelOAuth()}
                >
                  {t("platformAuth.oauthCancel")}
                </Button>
              )}
              {!config.isPending && !config.isFetching && !oauthAvailable && (
                <>
                  <LoginError
                    message={
                      config.error
                        ? platformAuthError(config.error)
                        : t("platformAuth.oauthUnavailable")
                    }
                  />
                  <Button
                    type="button"
                    variant="neutral"
                    className="w-full"
                    disabled={oauthBusy}
                    onClick={() => void config.refetch()}
                  >
                    {t("platformAuth.retryConfig")}
                  </Button>
                </>
              )}
              <details className="group rounded-control border border-border px-3 py-2">
                <summary className="flex cursor-pointer list-none items-center gap-2 text-caption text-fg-2 transition-colors hover:text-fg-1 [&::-webkit-details-marker]:hidden">
                  <Server className="h-3.5 w-3.5 shrink-0" />
                  <span className="shrink-0">{t("platformAuth.advanced")}</span>
                  <span className="ms-auto min-w-0 truncate font-mono text-[11px] text-fg-3">
                    {serverLabel}
                  </span>
                  <ChevronDown className="h-3.5 w-3.5 shrink-0 transition-transform group-open:rotate-180" />
                </summary>
                <div className="mt-2.5 space-y-3">
                  <div className="space-y-1.5">
                    <Label
                      htmlFor="platform-url"
                      className="text-caption text-fg-2"
                    >
                      {t("platformAuth.address")}
                    </Label>
                    <Input
                      id="platform-url"
                      type="url"
                      value={baseUrl}
                      disabled={oauthBusy}
                      onChange={(event) => {
                        setBaseUrl(event.target.value);
                        setError("");
                      }}
                      className="h-9 font-mono text-caption"
                    />
                  </div>
                </div>
              </details>
              {(error || auth.error) && (
                <LoginError message={error || auth.error} />
              )}
            </div>
          </div>
          <p className="mt-4 flex items-start justify-center gap-1.5 text-center text-caption text-fg-3">
            <ShieldCheck className="mt-[3px] h-3.5 w-3.5 shrink-0" />
            <span>{t("platformAuth.sessionHint")}</span>
          </p>
        </div>
      </main>
    </div>
  );
}

export function PlatformAccountMenu({ collapsed }: { collapsed: boolean }) {
  const auth = usePlatformAuth();
  const { t, i18n } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [refreshingBalance, setRefreshingBalance] = useState(false);
  if (!auth?.account) return null;
  const { user } = auth.account;
  // 平台钱包金额固定以美元计价，不跟随本地用量展示币种切换。
  const digits =
    Math.abs(user.balance) > 0 && Math.abs(user.balance) < 0.01 ? 6 : 2;
  const balance = new Intl.NumberFormat(
    i18n?.resolvedLanguage || i18n?.language || "zh-CN",
    {
      style: "currency",
      currency: "USD",
      currencyDisplay: "narrowSymbol",
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    },
  ).format(user.balance);
  const balanceDescription = `${t("platformAuth.balance")}: ${balance} USD · ${t("platformAuth.refreshBalance")}`;

  return (
    <div className="mx-2 mb-2 shrink-0 rounded-control border border-border p-2">
      <div
        className={`flex items-center gap-2 ${collapsed ? "justify-center" : ""}`}
      >
        {!collapsed && (
          <>
            <UserRound className="h-4 w-4 shrink-0 text-fg-2" />
            <div className="min-w-0 flex-1" title={user.email}>
              <p className="truncate text-caption font-medium">
                {user.username || user.email}
              </p>
              <p className="truncate text-caption text-fg-3">
                {t("platformAuth.account")}
              </p>
            </div>
          </>
        )}
        <button
          type="button"
          className="rounded-control p-1 text-fg-2 hover:bg-subtle"
          title={t("platformAuth.logout")}
          aria-label={t("platformAuth.logout")}
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void auth.logout().finally(() => setBusy(false));
          }}
        >
          <LogOut className="h-4 w-4" />
        </button>
        <PlatformAccountCleanupButton
          key={JSON.stringify([auth.account.baseUrl, user.id])}
          disabled={busy}
        />
      </div>
      <button
        type="button"
        className={`mt-2 flex w-full min-w-0 items-center rounded-control border-t border-border pt-2 text-caption text-fg-2 hover:text-fg-1 disabled:cursor-wait ${collapsed ? "justify-center" : "justify-between gap-2"}`}
        title={balanceDescription}
        aria-label={balanceDescription}
        aria-busy={refreshingBalance}
        disabled={busy || refreshingBalance}
        onClick={() => {
          setRefreshingBalance(true);
          void auth.refresh().finally(() => setRefreshingBalance(false));
        }}
      >
        {!collapsed && (
          <span className="flex shrink-0 items-center gap-1.5">
            <Wallet className="h-3.5 w-3.5" />
            {t("platformAuth.balance")}
          </span>
        )}
        <span className="flex min-w-0 items-center gap-1.5">
          <span
            className={`truncate font-medium tabular-nums text-fg-1 ${collapsed ? "text-[10px]" : ""}`}
          >
            {balance}
          </span>
          {!collapsed && (
            <RefreshCw
              className={`h-3 w-3 shrink-0 text-fg-3 ${refreshingBalance ? "animate-spin" : ""}`}
            />
          )}
        </span>
      </button>
      {!collapsed && auth.error && (
        <p
          role="status"
          className="mt-2 whitespace-normal text-caption text-danger"
        >
          {auth.error}
        </p>
      )}
    </div>
  );
}
