import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { Loader2, LogOut, RefreshCw, UserRound, Wallet } from "lucide-react";
import { toast } from "sonner";
import {
  platformAuthApi,
  platformAuthError,
  type SavedPlatformLogin,
} from "@/zhishu/api/platformAuth";
import { usePlatformAuth } from "@/zhishu/auth/PlatformAuthContext";
import { Button } from "@/components/ui/button";
import { Input, fieldClass } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { settingsApi } from "@/lib/api/index";
import { DRAG_REGION_ATTR, DRAG_REGION_STYLE, isMac } from "@/lib/platform";
import { WindowControls } from "@/components/shell/WindowControls";
import logo from "@/zhishu/assets/logo.png";

// Vite 根目录是 src/，可选的部署环境配置相应放在 src/.env.local。
const DEFAULT_API =
  import.meta.env.VITE_PLATFORM_API_URL || "http://127.0.0.1:3000";
const WEBSITE =
  import.meta.env.VITE_PLATFORM_WEB_URL || "http://127.0.0.1:3081";

/** 平台登录门禁：恢复期间等待验证，恢复失败提供重试入口，通过认证后才挂载工具界面。 */
export function PlatformAuthGate({ children }: { children: ReactNode }) {
  const auth = usePlatformAuth();
  const { t } = useTranslation();
  if (!auth) throw new Error("PlatformAuthProvider is required");
  if (auth.loading)
    return (
      <div
        className="flex h-screen items-center justify-center bg-surface text-fg-2"
        role="status"
      >
        {t("platformAuth.loading")}
      </div>
    );
  if (auth.recoveryPending) return <PlatformSessionRecovery />;
  return auth.account ? children : <PlatformLogin />;
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
    <div className="flex h-screen flex-col overflow-auto bg-surface text-fg-1">
      <div
        className="flex h-11 shrink-0 items-center justify-end px-3"
        {...DRAG_REGION_ATTR}
        style={DRAG_REGION_STYLE}
      >
        {!isMac() && (
          <div className="flex gap-1">
            <WindowControls />
          </div>
        )}
      </div>
      <main className="m-auto w-full max-w-sm space-y-4 px-6 pb-10">
        <h1 className="text-xl font-semibold">
          {t("platformAuth.restoreFailed")}
        </h1>
        <p className="text-body text-fg-2">{t("platformAuth.restoreHint")}</p>
        <p role="alert" className="text-body text-fg-2">
          {auth.error}
        </p>
        <div className="flex gap-2">
          <Button disabled={busy} onClick={() => void retry()}>
            {t("platformAuth.retryRestore")}
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => void signIn()}
          >
            {t("platformAuth.back")}
          </Button>
        </div>
      </main>
    </div>
  );
}

/** 登录表单与二次验证共用入口；账号历史可预填密码，仍需用户提交并完成平台验证。 */
export function PlatformLogin() {
  const auth = usePlatformAuth();
  const { t } = useTranslation();
  const [baseUrl, setBaseUrl] = useState(DEFAULT_API);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [requires2fa, setRequires2fa] = useState(false);
  const [history, setHistory] = useState<SavedPlatformLogin[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [rememberPassword, setRememberPassword] = useState(false);
  const [historyBusy, setHistoryBusy] = useState(true);
  const historyGeneration = useRef(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const disabled = busy || historyBusy;
  const selectLogin = async (record?: SavedPlatformLogin) => {
    const generation = ++historyGeneration.current;
    setSelectedId(record?.id || "");
    setBaseUrl(record?.baseUrl || DEFAULT_API);
    setEmail(record?.email || "");
    setPassword("");
    setRememberPassword(record?.rememberPassword || false);
    setError("");
    setHistoryBusy(true);
    try {
      if (record?.rememberPassword) {
        const saved = await platformAuthApi.savedPassword(record.id);
        if (generation === historyGeneration.current) {
          setPassword(saved || "");
          setRememberPassword(Boolean(saved));
        }
      }
    } catch (cause) {
      if (generation === historyGeneration.current)
        setError(platformAuthError(cause));
    } finally {
      if (generation === historyGeneration.current) setHistoryBusy(false);
    }
  };

  useEffect(() => {
    let active = true;
    void platformAuthApi
      .history()
      .then(async (records) => {
        if (!active) return;
        setHistory(records);
        await selectLogin(records[0]);
      })
      .catch((cause) => {
        if (active) {
          setError(platformAuthError(cause));
          setHistoryBusy(false);
        }
      });
    return () => {
      active = false;
      historyGeneration.current += 1;
    };
  }, []);

  const editIdentity = () => {
    if (selectedId) {
      setSelectedId("");
      setPassword("");
      setRememberPassword(false);
    }
  };

  const forgetPassword = async (checked: boolean) => {
    if (checked || !selectedId) {
      setRememberPassword(checked);
      return;
    }
    setHistoryBusy(true);
    setError("");
    try {
      await platformAuthApi.forgetPassword(selectedId);
      setRememberPassword(false);
      setHistory((records) =>
        records.map((record) =>
          record.id === selectedId
            ? { ...record, rememberPassword: false }
            : record,
        ),
      );
    } catch (cause) {
      setError(platformAuthError(cause));
    } finally {
      setHistoryBusy(false);
    }
  };

  const removeLogin = async () => {
    setHistoryBusy(true);
    setError("");
    try {
      await platformAuthApi.removeLogin(selectedId);
      setHistory((records) =>
        records.filter((record) => record.id !== selectedId),
      );
      await selectLogin();
    } catch (cause) {
      setError(platformAuthError(cause));
    } finally {
      setHistoryBusy(false);
    }
  };

  if (!auth) return null;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (disabled) return;
    setBusy(true);
    setError("");
    try {
      const result = requires2fa
        ? await platformAuthApi.login2fa(code)
        : await platformAuthApi.login(
            baseUrl,
            email,
            password,
            rememberPassword,
          );
      setPassword("");
      setCode("");
      if (result.status === "requires_2fa") setRequires2fa(true);
      else {
        if (result.persistenceWarning) toast.warning(result.persistenceWarning);
        auth.acceptLogin(result);
      }
    } catch (cause) {
      setError(platformAuthError(cause));
    } finally {
      setBusy(false);
    }
  };
  const openWebsite = async (path: string) => {
    try {
      await settingsApi.openExternal(`${WEBSITE.replace(/\/$/, "")}${path}`);
    } catch (cause) {
      setError(platformAuthError(cause));
    }
  };

  return (
    <div className="flex h-screen flex-col overflow-auto bg-surface text-fg-1">
      <div
        className="flex h-11 shrink-0 items-center justify-end px-3"
        {...DRAG_REGION_ATTR}
        style={DRAG_REGION_STYLE}
      >
        {!isMac() && (
          <div className="flex gap-1">
            <WindowControls />
          </div>
        )}
      </div>
      <main className="m-auto w-full max-w-sm px-6 pb-10">
        <div className="mb-5 flex h-10 w-20 items-center justify-center overflow-hidden">
          <img
            src={logo}
            alt=""
            className="h-20 w-20 shrink-0 object-contain dark:invert"
          />
        </div>
        <h1 className="text-2xl font-semibold">{t("platformAuth.title")}</h1>
        <p className="mb-7 mt-2 text-body text-fg-2">
          {t("platformAuth.subtitle")}
        </p>
        <form
          onSubmit={(event) => void submit(event)}
          className="space-y-4"
          aria-busy={disabled}
        >
          {requires2fa ? (
            <div className="space-y-2">
              <Label htmlFor="platform-totp">{t("platformAuth.totp")}</Label>
              <Input
                id="platform-totp"
                autoFocus
                required
                inputMode="numeric"
                autoComplete="one-time-code"
                pattern="[0-9]{6}"
                maxLength={6}
                value={code}
                disabled={disabled}
                onChange={(event) => setCode(event.target.value)}
              />
              <p className="text-caption text-fg-2">
                {t("platformAuth.totpHint")}
              </p>
            </div>
          ) : (
            <>
              {history.length > 0 && (
                <div className="space-y-2">
                  <Label htmlFor="platform-history">
                    {t("platformAuth.loginHistory")}
                  </Label>
                  <div className="flex gap-2">
                    <select
                      id="platform-history"
                      className={`${fieldClass} h-8 min-w-0 flex-1`}
                      value={selectedId}
                      disabled={disabled}
                      onChange={(event) =>
                        void selectLogin(
                          history.find(
                            (record) => record.id === event.target.value,
                          ),
                        )
                      }
                    >
                      <option value="">{t("platformAuth.otherAccount")}</option>
                      {history.map((record) => (
                        <option key={record.id} value={record.id}>
                          {record.email} · {record.baseUrl}
                        </option>
                      ))}
                    </select>
                    <Button
                      type="button"
                      variant="outline"
                      disabled={disabled || !selectedId}
                      onClick={() => void removeLogin()}
                    >
                      {t("platformAuth.removeLogin")}
                    </Button>
                  </div>
                </div>
              )}
              <div className="space-y-2">
                <Label htmlFor="platform-url">
                  {t("platformAuth.address")}
                </Label>
                <Input
                  id="platform-url"
                  type="url"
                  required
                  value={baseUrl}
                  disabled={disabled}
                  onChange={(event) => {
                    editIdentity();
                    setBaseUrl(event.target.value);
                  }}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="platform-email">
                  {t("platformAuth.email")}
                </Label>
                <Input
                  id="platform-email"
                  type="email"
                  required
                  autoComplete="username"
                  value={email}
                  disabled={disabled}
                  onChange={(event) => {
                    editIdentity();
                    setEmail(event.target.value);
                  }}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="platform-password">
                  {t("platformAuth.password")}
                </Label>
                <Input
                  id="platform-password"
                  type="password"
                  required
                  autoComplete="current-password"
                  value={password}
                  disabled={disabled}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </div>
              <div className="flex items-center gap-2">
                <Checkbox
                  id="platform-remember"
                  checked={rememberPassword}
                  disabled={disabled}
                  onCheckedChange={(checked) => void forgetPassword(checked)}
                />
                <Label htmlFor="platform-remember">
                  {t("platformAuth.rememberPassword")}
                </Label>
              </div>
            </>
          )}
          {(error || auth.error) && (
            <p role="alert" className="text-body text-danger">
              {error || auth.error}
            </p>
          )}
          <Button
            type="submit"
            disabled={disabled}
            className="w-full bg-action-text text-surface hover:bg-action-text hover:brightness-90"
          >
            {busy && <Loader2 className="me-2 h-4 w-4 animate-spin" />}
            {t(requires2fa ? "platformAuth.verify" : "platformAuth.login")}
          </Button>
          {requires2fa && (
            <Button
              type="button"
              variant="ghost"
              className="w-full"
              disabled={disabled}
              onClick={() => {
                setBusy(true);
                void platformAuthApi
                  .cancelLogin()
                  .then(() => {
                    setRequires2fa(false);
                    setCode("");
                    setError("");
                  })
                  .catch((cause) => setError(platformAuthError(cause)))
                  .finally(() => setBusy(false));
              }}
            >
              {t("platformAuth.back")}
            </Button>
          )}
        </form>
        <div className="mt-3 flex justify-between">
          <Button
            variant="link"
            disabled={disabled}
            onClick={() => void openWebsite("/register")}
          >
            {t("platformAuth.register")}
          </Button>
          <Button
            variant="link"
            disabled={disabled}
            onClick={() => void openWebsite("/forgot-password")}
          >
            {t("platformAuth.forgot")}
          </Button>
        </div>
        <p className="mt-5 text-caption text-fg-3">
          {t("platformAuth.sessionHint")}
        </p>
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
