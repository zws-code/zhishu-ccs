import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  PlatformAuthGate,
  PlatformAccountMenu,
} from "@/zhishu/auth/PlatformLogin";
import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/zhishu/api/platformModels";
import { platformCatalogKeys } from "@/zhishu/models/query/platformCatalogKeys";
import {
  PlatformAuthProvider,
  usePlatformAuth,
} from "@/zhishu/auth/PlatformAuthContext";
import {
  platformAuthApi,
  type PlatformAccount,
  type PlatformCleanupReport,
} from "@/zhishu/api/platformAuth";
import { toast } from "sonner";
import { settingsApi } from "@/lib/api/index";
import zh from "@/i18n/locales/zh.json";

vi.mock("@/zhishu/api/platformAuth", async (original) => ({
  ...(await original<typeof import("@/zhishu/api/platformAuth")>()),
  platformAuthApi: {
    config: vi.fn(),
    account: vi.fn(),
    history: vi.fn(),
    savedPassword: vi.fn(),
    forgetPassword: vi.fn(),
    removeLogin: vi.fn(),
    cancelLogin: vi.fn(),
    login: vi.fn(),
    login2fa: vi.fn(),
    logout: vi.fn(),
    logoutAndClear: vi.fn(),
  },
}));
vi.mock("@/zhishu/api/platformModels", () => ({
  platformModelsApi: { syncCatalog: vi.fn(), sync: vi.fn() },
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | Record<string, unknown>) => {
      const value = key
        .split(".")
        .reduce<unknown>(
          (object, part) =>
            object && typeof object === "object"
              ? (object as Record<string, unknown>)[part]
              : undefined,
          zh,
        );
      const text =
        typeof value === "string"
          ? value
          : typeof options === "string"
            ? options
            : String(options?.defaultValue ?? key);
      return text.replace(/\{\{(\w+)\}\}/g, (_, name) =>
        String(typeof options === "object" ? (options[name] ?? "") : ""),
      );
    },
  }),
}));
vi.mock("sonner", () => ({ toast: { warning: vi.fn(), error: vi.fn() } }));
vi.mock("@/lib/api/index", () => ({ settingsApi: { openExternal: vi.fn() } }));
vi.mock("@/components/shell/WindowControls", () => ({
  WindowControls: () => null,
}));

const account: PlatformAccount = {
  baseUrl: "http://127.0.0.1:3000",
  user: {
    id: 1,
    email: "user@example.com",
    username: "测试账户",
    role: "user",
    balance: 10,
  },
};
const catalog: PlatformCatalog = {
  baseUrl: account.baseUrl,
  userId: account.user.id,
  syncedAt: 1,
  groups: [],
  warnings: [],
};
function CatalogStatus() {
  const auth = usePlatformAuth()!;
  return (
    <>
      <p data-testid="catalog-status">
        {auth.catalogSyncError || (auth.catalogSyncing ? "syncing" : "idle")}
      </p>
      <button onClick={() => void auth.syncCatalog().catch(() => undefined)}>
        重试同步目录
      </button>
    </>
  );
}
function renderGate() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <PlatformAuthProvider>
        <PlatformAuthGate>
          <p>工具主页</p>
          <CatalogStatus />
          <PlatformAccountMenu collapsed={false} />
        </PlatformAuthGate>
      </PlatformAuthProvider>
    </QueryClientProvider>,
  );
  return client;
}
async function submitCredentials() {
  await waitFor(() => expect(screen.getByLabelText("邮箱")).toBeEnabled());
  fireEvent.change(screen.getByLabelText("邮箱"), {
    target: { value: "user@example.com" },
  });
  fireEvent.change(screen.getByLabelText("密码"), {
    target: { value: "secret-password" },
  });
  await act(async () => {
    fireEvent.submit(
      screen.getByRole("button", { name: "登录" }).closest("form")!,
    );
  });
}

beforeEach(() => {
  vi.mocked(platformAuthApi.config)
    .mockReset()
    .mockResolvedValue({
      schema_version: 1,
      site_name: "",
      api_base_url: "http://127.0.0.1:3000",
      login_methods: ["password"],
      totp_enabled: true,
      registration_enabled: true,
      password_reset_enabled: true,
      register_url: "https://portal.test/register",
      password_reset_url: "https://portal.test/reset",
      subscriptions_url: null,
      turnstile_enabled: false,
      tencent_captcha_enabled: false,
      aliyun_captcha_enabled: false,
    });
  vi.mocked(platformModelsApi.syncCatalog)
    .mockReset()
    .mockResolvedValue(catalog);
  vi.mocked(platformModelsApi.sync)
    .mockReset()
    .mockResolvedValue({ added: 0, removed: 0, skipped: 0, warnings: [] });
  vi.mocked(platformAuthApi.history).mockReset().mockResolvedValue([]);
  vi.mocked(platformAuthApi.savedPassword).mockReset().mockResolvedValue(null);
  vi.mocked(platformAuthApi.forgetPassword)
    .mockReset()
    .mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.removeLogin)
    .mockReset()
    .mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.cancelLogin)
    .mockReset()
    .mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.account).mockReset().mockResolvedValue(null);
  vi.mocked(platformAuthApi.login).mockReset();
  vi.mocked(platformAuthApi.login2fa).mockReset();
  vi.mocked(platformAuthApi.logout).mockReset().mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.logoutAndClear).mockReset();
});

describe("platform login", () => {
  it("uses configured web entries and hides them when disabled", async () => {
    const config = await platformAuthApi.config(account.baseUrl);
    vi.mocked(platformAuthApi.config).mockResolvedValue({
      ...config,
      registration_enabled: false,
      site_name: "Client test platform",
    });
    renderGate();
    await screen.findByText("Client test platform");
    expect(
      screen.queryByRole("button", { name: zh.platformAuth.register }),
    ).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: zh.platformAuth.forgot }),
    );
    await waitFor(() =>
      expect(settingsApi.openExternal).toHaveBeenCalledWith(
        "https://portal.test/reset",
      ),
    );
  });

  it("does not retain links from a previous platform while config is loading", async () => {
    renderGate();
    await screen.findByRole("button", { name: zh.platformAuth.register });
    vi.mocked(platformAuthApi.config).mockImplementation(
      () => new Promise(() => {}),
    );
    const address = screen.getByDisplayValue("http://127.0.0.1:3000");
    fireEvent.change(address, { target: { value: "https://another.test" } });
    expect(
      screen.queryByRole("button", { name: zh.platformAuth.register }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: zh.platformAuth.forgot }),
    ).not.toBeInTheDocument();
  });

  it("gates the app and passes the development backend URL to Rust", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    await submitCredentials();
    await screen.findByText("工具主页");
    expect(platformAuthApi.login).toHaveBeenCalledWith(
      "http://127.0.0.1:3000",
      "user@example.com",
      "secret-password",
      false,
    );
    expect(screen.getByText("测试账户")).toBeInTheDocument();
    expect(Object.values(localStorage)).not.toContain("secret-password");
  });

  it("requires TOTP completion before mounting the app", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "requires_2fa",
    });
    vi.mocked(platformAuthApi.login2fa).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    await submitCredentials();
    const input = await screen.findByLabelText("二次验证码");
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    fireEvent.change(input, { target: { value: "123456" } });
    fireEvent.submit(
      screen.getByRole("button", { name: "验证并登录" }).closest("form")!,
    );
    await screen.findByText("工具主页");
    expect(platformAuthApi.login2fa).toHaveBeenCalledWith("123456");
  });

  it("returns from TOTP to an empty password field", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "requires_2fa",
    });
    renderGate();
    await submitCredentials();
    fireEvent.click(
      await screen.findByRole("button", { name: "返回邮箱登录" }),
    );
    await waitFor(() => expect(screen.getByLabelText("密码")).toHaveValue(""));
    expect(platformAuthApi.cancelLogin).toHaveBeenCalledOnce();
  });

  it("shows failed login without opening the app", async () => {
    vi.mocked(platformAuthApi.login).mockRejectedValue({
      code: "UNAUTHORIZED",
      message: "邮箱或密码错误",
    });
    renderGate();
    await submitCredentials();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "邮箱或密码错误",
    );
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeEnabled();
  });

  it("restores the recent account and its password without logging in automatically", async () => {
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "one",
        baseUrl: "https://one.example.com",
        email: "one@example.com",
        rememberPassword: true,
      },
      {
        id: "two",
        baseUrl: "https://two.example.com",
        email: "two@example.com",
        rememberPassword: false,
      },
    ]);
    vi.mocked(platformAuthApi.savedPassword).mockResolvedValue("saved-secret");
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("密码")).toHaveValue("saved-secret"),
    );
    expect(screen.getByLabelText("邮箱")).toHaveValue("one@example.com");
    expect(screen.getByLabelText("平台后端地址")).toHaveValue(
      "https://one.example.com",
    );
    expect(screen.getByLabelText("记住密码")).toBeChecked();
    expect(platformAuthApi.login).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("登录账号记录"), {
      target: { value: "two" },
    });
    await waitFor(() =>
      expect(screen.getByLabelText("邮箱")).toHaveValue("two@example.com"),
    );
    expect(screen.getByLabelText("平台后端地址")).toHaveValue(
      "https://two.example.com",
    );
    expect(screen.getByLabelText("密码")).toHaveValue("");
    expect(screen.getByLabelText("记住密码")).not.toBeChecked();
    expect(platformAuthApi.savedPassword).toHaveBeenCalledOnce();
  });

  it("sends the remember password preference when signing in", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("记住密码")).toBeEnabled(),
    );
    fireEvent.click(screen.getByLabelText("记住密码"));
    fireEvent.change(screen.getByLabelText("邮箱"), {
      target: { value: "user@example.com" },
    });
    fireEvent.change(screen.getByLabelText("密码"), {
      target: { value: "remember-secret" },
    });
    fireEvent.submit(
      screen.getByRole("button", { name: "登录" }).closest("form")!,
    );
    await screen.findByText("工具主页");
    expect(platformAuthApi.login).toHaveBeenCalledWith(
      "http://127.0.0.1:3000",
      "user@example.com",
      "remember-secret",
      true,
    );
    expect(JSON.stringify(localStorage)).not.toContain("remember-secret");
  });

  it("forgets the saved password immediately when unchecked", async () => {
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "one",
        baseUrl: "https://one.example.com",
        email: "one@example.com",
        rememberPassword: true,
      },
    ]);
    vi.mocked(platformAuthApi.savedPassword).mockResolvedValue("saved-secret");
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("密码")).toHaveValue("saved-secret"),
    );
    fireEvent.click(screen.getByLabelText("记住密码"));
    await waitFor(() =>
      expect(screen.getByLabelText("记住密码")).not.toBeChecked(),
    );
    expect(platformAuthApi.forgetPassword).toHaveBeenCalledWith("one");
    expect(platformAuthApi.login).not.toHaveBeenCalled();
  });

  it("deletes an account record and clears its filled password", async () => {
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "one",
        baseUrl: "https://one.example.com",
        email: "one@example.com",
        rememberPassword: true,
      },
    ]);
    vi.mocked(platformAuthApi.savedPassword).mockResolvedValue("saved-secret");
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("密码")).toHaveValue("saved-secret"),
    );
    fireEvent.click(screen.getByRole("button", { name: "删除记录" }));
    await waitFor(() =>
      expect(screen.queryByLabelText("登录账号记录")).not.toBeInTheDocument(),
    );
    expect(platformAuthApi.removeLogin).toHaveBeenCalledWith("one");
    expect(screen.getByLabelText("邮箱")).toHaveValue("");
    expect(screen.getByLabelText("密码")).toHaveValue("");
  });

  it("clears a restored password when the platform address is edited", async () => {
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "one",
        baseUrl: "https://one.example.com",
        email: "one@example.com",
        rememberPassword: true,
      },
    ]);
    vi.mocked(platformAuthApi.savedPassword).mockResolvedValue("saved-secret");
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("密码")).toHaveValue("saved-secret"),
    );
    fireEvent.change(screen.getByLabelText("平台后端地址"), {
      target: { value: "https://other.example.com" },
    });
    expect(screen.getByLabelText("密码")).toHaveValue("");
    expect(screen.getByLabelText("记住密码")).not.toBeChecked();
  });

  it("allows manual login if reading saved accounts fails", async () => {
    vi.mocked(platformAuthApi.history).mockRejectedValue({
      code: "LOGIN_HISTORY",
      message: "无法读取登录记录",
    });
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "无法读取登录记录",
    );
    await submitCredentials();
    await screen.findByText("工具主页");
  });

  it("shows every saved server and signs in to an edited server address", async () => {
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "foreign",
        baseUrl: "https://old.example.com",
        email: "old@example.com",
        rememberPassword: false,
      },
      {
        id: "current",
        baseUrl: "http://127.0.0.1:3000",
        email: "current@example.com",
        rememberPassword: false,
      },
    ]);
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    await waitFor(() =>
      expect(screen.getByLabelText("邮箱")).toHaveValue("old@example.com"),
    );
    expect(
      screen.getByRole("option", {
        name: "old@example.com · https://old.example.com",
      }),
    ).toBeInTheDocument();
    const address = screen.getByLabelText("平台后端地址");
    expect(address).toHaveValue("https://old.example.com");
    fireEvent.change(address, {
      target: { value: "https://edited.example.com" },
    });
    await submitCredentials();
    expect(platformAuthApi.login).toHaveBeenCalledWith(
      "https://edited.example.com",
      "user@example.com",
      "secret-password",
      false,
    );
  });

  it("shows a storage warning without rejecting a successful login", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
      persistenceWarning: "无法保存密码",
    });
    renderGate();
    await submitCredentials();
    await screen.findByText("工具主页");
    expect(toast.warning).toHaveBeenCalledWith("无法保存密码");
  });

  it("clears the UI on logout even when server revocation fails", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logout).mockRejectedValue(new Error("offline"));
    renderGate();
    fireEvent.click(
      await screen.findByRole("button", { name: "退出平台登录" }),
    );
    await screen.findByLabelText("邮箱");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "本机已退出登录",
    );
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
  });

  it("detects session invalidation when the window becomes visible", async () => {
    vi.mocked(platformAuthApi.account)
      .mockResolvedValueOnce(account)
      .mockResolvedValue(null);
    renderGate();
    await screen.findByText("工具主页");
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await screen.findByLabelText("邮箱");
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
  });

  it("retains the current session on a temporary network error", async () => {
    vi.mocked(platformAuthApi.account)
      .mockResolvedValueOnce(account)
      .mockRejectedValue({ code: "NETWORK", message: "网络不可用" });
    renderGate();
    await screen.findByText("工具主页");
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("网络不可用"),
    );
    expect(screen.getByText("工具主页")).toBeInTheDocument();
  });
  it("restores an existing session without submitting a password or TOTP", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    renderGate();
    await screen.findByText("工具主页");
    expect(platformAuthApi.account).toHaveBeenCalledWith(true);
    expect(platformAuthApi.login).not.toHaveBeenCalled();
    expect(platformAuthApi.login2fa).not.toHaveBeenCalled();
    expect(platformAuthApi.savedPassword).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
  });

  it("keeps a failed startup restoration separate from sign in and allows retry", async () => {
    vi.mocked(platformAuthApi.account)
      .mockRejectedValueOnce({ code: "NETWORK", message: "网络不可用" })
      .mockResolvedValue(account);
    renderGate();
    expect(await screen.findByRole("alert")).toHaveTextContent("网络不可用");
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    expect(platformAuthApi.logout).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "重试恢复登录" }));
    await screen.findByText("工具主页");
    expect(platformAuthApi.login).not.toHaveBeenCalled();
  });

  it("retries restoration when the connection returns", async () => {
    vi.mocked(platformAuthApi.account)
      .mockRejectedValueOnce({ code: "NETWORK", message: "网络不可用" })
      .mockResolvedValue(account);
    renderGate();
    await screen.findByRole("alert");
    await act(async () => {
      window.dispatchEvent(new Event("online"));
    });
    await screen.findByText("工具主页");
  });

  it("allows clearing a failed saved session before returning to sign in", async () => {
    vi.mocked(platformAuthApi.account).mockRejectedValue({
      code: "SESSION_STORE",
      message: "凭据库被锁定",
    });
    renderGate();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "返回邮箱登录" }));
    await screen.findByLabelText("密码");
    expect(platformAuthApi.logout).toHaveBeenCalledOnce();
    expect(platformAuthApi.login).not.toHaveBeenCalled();
  });

  it("reports hidden window checks as background activity", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    renderGate();
    await screen.findByText("工具主页");
    const visibility = vi
      .spyOn(document, "visibilityState", "get")
      .mockReturnValue("hidden");
    // Manual account refresh in a hidden window must also be marked inactive.
    fireEvent.click(screen.getByRole("button", { name: /点击刷新余额/ }));
    await waitFor(() =>
      expect(platformAuthApi.account).toHaveBeenLastCalledWith(false),
    );
    visibility.mockRestore();
  });

  it("clears user-specific cached data when a saved session is invalidated", async () => {
    vi.mocked(platformAuthApi.account)
      .mockResolvedValueOnce(account)
      .mockResolvedValue(null);
    const client = renderGate();
    await screen.findByText("工具主页");
    client.setQueryData(["providers", "claude"], { private: "old account" });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await screen.findByLabelText("密码");
    expect(client.getQueryData(["providers", "claude"])).toBeUndefined();
  });

  it("does not allow a new login while saved credentials are still being cleared", async () => {
    let finishLogout!: () => void;
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logout).mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finishLogout = resolve;
        }),
    );
    renderGate();
    fireEvent.click(
      await screen.findByRole("button", { name: "退出平台登录" }),
    );
    await waitFor(() => expect(platformAuthApi.logout).toHaveBeenCalledOnce());
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    await act(async () => {
      finishLogout();
    });
    await screen.findByLabelText("密码");
  });
});

describe("catalog sync after verified login", () => {
  it("keeps the account usable when automatic provider sync fails", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.sync).mockRejectedValue({
      code: "CONFIG",
      message: "工具配置失败",
    });
    renderGate();
    await waitFor(() =>
      expect(screen.getByTestId("catalog-status")).toHaveTextContent("idle"),
    );
    expect(toast.error).toHaveBeenCalledWith("工具配置失败");
    expect(screen.getByText("工具主页")).toBeInTheDocument();
    expect(platformAuthApi.logout).not.toHaveBeenCalled();
  });

  it("shows provider compatibility warnings only as a toast, not catalog errors", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.sync).mockResolvedValue({
      added: 1,
      removed: 0,
      skipped: 0,
      warnings: ["Gemini CLI 不能使用该分组的 GPT/Claude 模型"],
    });
    renderGate();
    await waitFor(() =>
      expect(screen.getByTestId("catalog-status")).toHaveTextContent("idle"),
    );
    expect(toast.warning).toHaveBeenCalledWith(
      "Gemini CLI 不能使用该分组的 GPT/Claude 模型",
    );
    expect(screen.getByText("工具主页")).toBeInTheDocument();
  });

  it("syncs once on restoration and does not resync during account checks", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    const client = renderGate();
    await screen.findByText("工具主页");
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce(),
    );
    await waitFor(() =>
      expect(
        client.getQueryData(
          platformCatalogKeys.account(account.baseUrl, account.user.id),
        ),
      ).toEqual(catalog),
    );
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("online"));
    });
    expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce();
    expect(platformModelsApi.sync).toHaveBeenCalledOnce();
    expect(platformModelsApi.sync).toHaveBeenCalledWith();
  });
  it("does not sync before the second authentication factor completes", async () => {
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "requires_2fa",
    });
    vi.mocked(platformAuthApi.login2fa).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    await submitCredentials();
    const input = await screen.findByLabelText("二次验证码");
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "123456" } });
    fireEvent.submit(
      screen.getByRole("button", { name: "验证并登录" }).closest("form")!,
    );
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce(),
    );
  });
  it("allows retry after sync failure without losing the authenticated account", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.syncCatalog)
      .mockRejectedValueOnce({ code: "NETWORK", message: "目录暂时不可用" })
      .mockResolvedValue(catalog);
    renderGate();
    await waitFor(() =>
      expect(screen.getByTestId("catalog-status")).toHaveTextContent(
        "目录暂时不可用",
      ),
    );
    expect(screen.getByText("工具主页")).toBeInTheDocument();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("重试同步目录"));
    await waitFor(() =>
      expect(screen.getByTestId("catalog-status")).toHaveTextContent("idle"),
    );
    expect(platformModelsApi.syncCatalog).toHaveBeenCalledTimes(2);
  });
  it("ignores an old sync after logout and a new login to the same account", async () => {
    let finish!: (value: PlatformCatalog) => void;
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.login).mockResolvedValue({
      status: "authenticated",
      account,
    });
    vi.mocked(platformModelsApi.syncCatalog)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      )
      .mockResolvedValue({ ...catalog, syncedAt: 2 });
    const client = renderGate();
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce(),
    );
    fireEvent.click(screen.getByRole("button", { name: "退出平台登录" }));
    await submitCredentials();
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledTimes(2),
    );
    await act(async () => {
      finish(catalog);
    });
    expect(
      client.getQueryData(
        platformCatalogKeys.account(account.baseUrl, account.user.id),
      ),
    ).toEqual({ ...catalog, syncedAt: 2 });
  });
  it("coalesces manual retry with an automatic sync already in progress", async () => {
    let finish!: (value: PlatformCatalog) => void;
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.syncCatalog).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    renderGate();
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce(),
    );
    fireEvent.click(screen.getByText("重试同步目录"));
    expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce();
    await act(async () => finish(catalog));
  });
});

const cleanupReport: PlatformCleanupReport = {
  baseUrl: account.baseUrl,
  userId: account.user.id,
  email: account.user.email,
  providers: {
    removed: 2,
    skipped: 0,
    warnings: [],
    nativeFilesChanged: 0,
    apps: [],
  },
  providerError: null,
  catalog: { completed: true, error: null },
  session: { completed: true, error: null },
  serverLogout: { completed: true, error: null },
};

async function openCleanup() {
  fireEvent.click(
    await screen.findByRole("button", { name: zh.platformCleanup.action }),
  );
  return screen.getByRole("alertdialog");
}

describe("account cleanup logout", () => {
  it("requires explicit confirmation and cancel preserves login", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    renderGate();
    const dialog = await openCleanup();
    expect(
      within(dialog).getByText(zh.platformCleanup.scope),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(zh.platformCleanup.retained),
    ).toBeInTheDocument();
    expect(platformAuthApi.logoutAndClear).not.toHaveBeenCalled();
    fireEvent.click(
      within(dialog).getByRole("button", { name: zh.common.cancel }),
    );
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByText("工具主页")).toBeInTheDocument();
    expect(platformAuthApi.logout).not.toHaveBeenCalled();
  });

  it("shows a dismissible result dialog and preserves saved login details", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logoutAndClear).mockResolvedValue(cleanupReport);
    vi.mocked(platformAuthApi.history).mockResolvedValue([
      {
        id: "saved-account",
        baseUrl: "https://saved-platform.test",
        email: account.user.email,
        rememberPassword: true,
      },
    ]);
    vi.mocked(platformAuthApi.savedPassword).mockResolvedValue(
      "remembered-password",
    );
    const client = renderGate();
    const dialog = await openCleanup();
    client.setQueryData(["providers", "claude"], { private: "old-account" });
    fireEvent.click(
      within(dialog).getByRole("button", { name: zh.platformCleanup.confirm }),
    );
    await screen.findByText(zh.platformCleanup.complete);
    expect(platformAuthApi.logoutAndClear).toHaveBeenCalledOnce();
    expect(platformAuthApi.logoutAndClear).toHaveBeenCalledWith(
      account.baseUrl,
      account.user.id,
    );
    expect(platformAuthApi.logout).not.toHaveBeenCalled();
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    expect(screen.getByLabelText("密码")).toBeInTheDocument();
    expect(client.getQueryData(["providers", "claude"])).toBeUndefined();
    const resultDialog = screen.getByRole("dialog", {
      name: zh.platformCleanup.result,
    });
    expect(
      within(resultDialog).getByText(zh.platformCleanup.complete),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("密码").closest("main")).not.toContainElement(
      resultDialog,
    );
    const appDetails = within(resultDialog)
      .getByText(zh.platformCatalog.deleteAllResult)
      .closest("details");
    expect(appDetails).not.toHaveAttribute("open");
    fireEvent.click(
      within(resultDialog).getByRole("button", { name: zh.common.close }),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(
      screen.queryByText(zh.platformCleanup.complete),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByLabelText("密码")).toHaveValue("remembered-password"),
    );
    expect(screen.getByLabelText("邮箱")).toHaveValue(account.user.email);
    expect(screen.getByLabelText(zh.platformAuth.address)).toHaveValue(
      "https://saved-platform.test",
    );
    expect(platformAuthApi.removeLogin).not.toHaveBeenCalled();
    expect(platformAuthApi.forgetPassword).not.toHaveBeenCalled();
  });

  it("reports partial failures and retained providers without claiming complete cleanup", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logoutAndClear).mockResolvedValue({
      ...cleanupReport,
      providers: {
        ...cleanupReport.providers!,
        skipped: 1,
        warnings: ["没有可用备用供应商"],
      },
      session: {
        completed: false,
        error: { code: "CREDENTIAL_STORE", message: "请解锁系统凭据库" },
      },
      serverLogout: {
        completed: false,
        error: { code: "NETWORK", message: "服务端退出暂时不可用" },
      },
    });
    renderGate();
    const dialog = await openCleanup();
    fireEvent.click(
      within(dialog).getByRole("button", { name: zh.platformCleanup.confirm }),
    );
    await screen.findByText(zh.platformCleanup.partial);
    expect(screen.getByText("没有可用备用供应商")).toBeInTheDocument();
    expect(screen.getByText("请解锁系统凭据库")).toBeInTheDocument();
    expect(screen.getByText("服务端退出暂时不可用")).toBeInTheDocument();
    expect(
      screen.queryByText(zh.platformCleanup.complete),
    ).not.toBeInTheDocument();
  });

  it("blocks login and polling during cleanup and discards late catalog responses", async () => {
    let finishCleanup!: (result: PlatformCleanupReport) => void;
    let finishCatalog!: (result: PlatformCatalog) => void;
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.syncCatalog).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishCatalog = resolve;
        }),
    );
    vi.mocked(platformAuthApi.logoutAndClear).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishCleanup = resolve;
        }),
    );
    const client = renderGate();
    const dialog = await openCleanup();
    fireEvent.click(
      within(dialog).getByRole("button", { name: zh.platformCleanup.confirm }),
    );
    await waitFor(() =>
      expect(platformAuthApi.logoutAndClear).toHaveBeenCalledOnce(),
    );
    expect(screen.getByText(zh.platformCleanup.working)).toBeInTheDocument();
    const calls = vi.mocked(platformAuthApi.account).mock.calls.length;
    await act(async () => {
      window.dispatchEvent(new Event("online"));
      document.dispatchEvent(new Event("visibilitychange"));
      finishCatalog(catalog);
    });
    expect(platformAuthApi.account).toHaveBeenCalledTimes(calls);
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    expect(
      client.getQueryData(
        platformCatalogKeys.account(account.baseUrl, account.user.id),
      ),
    ).toBeUndefined();
    await act(async () => finishCleanup(cleanupReport));
    await screen.findByLabelText("密码");
  });

  it("does not claim logout when the cleanup IPC rejects before identity validation", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logoutAndClear).mockRejectedValue({
      code: "UNAUTHORIZED",
      message: "账号已经改变",
    });
    renderGate();
    const dialog = await openCleanup();
    fireEvent.click(
      within(dialog).getByRole("button", { name: zh.platformCleanup.confirm }),
    );
    await screen.findByText("账号已经改变");
    expect(
      screen.queryByText(zh.platformCleanup.complete),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: zh.platformAuth.retryRestore }),
    ).toBeInTheDocument();
  });

  it("ordinary logout never calls account cleanup", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    renderGate();
    fireEvent.click(
      await screen.findByRole("button", { name: zh.platformAuth.logout }),
    );
    await screen.findByLabelText("密码");
    expect(platformAuthApi.logout).toHaveBeenCalledOnce();
    expect(platformAuthApi.logoutAndClear).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText(zh.platformCleanup.result),
    ).not.toBeInTheDocument();
  });
});
