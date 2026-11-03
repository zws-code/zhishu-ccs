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
import zh from "@/i18n/locales/zh.json";

vi.mock("@/zhishu/api/platformAuth", async (original) => ({
  ...(await original<typeof import("@/zhishu/api/platformAuth")>()),
  platformAuthApi: {
    config: vi.fn(),
    account: vi.fn(),
    cancelLogin: vi.fn(),
    loginOAuth: vi.fn(),
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

function oauthConfig() {
  return {
    schema_version: 1,
    site_name: "Zhishu",
    api_base_url: account.baseUrl,
    login_methods: ["password", "oauth"],
    subscriptions_url: null,
    oauth_authorize_url: "https://portal.test/oauth/authorize",
    oauth_client_id: "zhishu-ccs",
    oauth_scopes: ["client"],
  };
}
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
async function submitOAuth() {
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "登录智枢" })).toBeEnabled(),
  );
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "登录智枢" }));
  });
}

beforeEach(() => {
  vi.mocked(platformAuthApi.config)
    .mockReset()
    .mockResolvedValue(oauthConfig());
  vi.mocked(platformModelsApi.syncCatalog)
    .mockReset()
    .mockResolvedValue(catalog);
  vi.mocked(platformModelsApi.sync)
    .mockReset()
    .mockResolvedValue({ added: 0, removed: 0, skipped: 0, warnings: [] });
  vi.mocked(platformAuthApi.cancelLogin)
    .mockReset()
    .mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.account).mockReset().mockResolvedValue(null);
  vi.mocked(platformAuthApi.loginOAuth).mockReset();
  vi.mocked(platformAuthApi.logout).mockReset().mockResolvedValue(undefined);
  vi.mocked(platformAuthApi.logoutAndClear).mockReset();
});

describe("platform login", () => {
  it("only offers browser authorization even when password login is advertised", async () => {
    renderGate();
    await screen.findByRole("button", { name: "登录智枢" });
    expect(screen.queryByLabelText("邮箱")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("记住密码")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "账号密码登录" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("忘记密码")).not.toBeInTheDocument();
  });

  it.each(["methods", "url", "client", "legacy"])(
    "blocks unsupported OAuth config (%s) without falling back to passwords",
    async (missing) => {
      const config = oauthConfig();
      if (missing === "methods") config.login_methods = ["password"];
      if (missing === "url") config.oauth_authorize_url = "";
      if (missing === "client") config.oauth_client_id = "";
      if (missing === "legacy") {
        Reflect.deleteProperty(config, "oauth_authorize_url");
        Reflect.deleteProperty(config, "oauth_client_id");
      }
      vi.mocked(platformAuthApi.config).mockResolvedValue(config);
      renderGate();
      expect(await screen.findByRole("alert")).toHaveTextContent(
        zh.platformAuth.oauthUnavailable,
      );
      expect(screen.getByRole("button", { name: "登录智枢" })).toBeDisabled();
      expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
      expect(platformAuthApi.loginOAuth).not.toHaveBeenCalled();
    },
  );

  it("retries config discovery after a temporary failure", async () => {
    vi.mocked(platformAuthApi.config)
      .mockRejectedValueOnce({ code: "NETWORK", message: "网络不可用" })
      .mockResolvedValue(oauthConfig());
    renderGate();
    expect(await screen.findByRole("alert")).toHaveTextContent("网络不可用");
    fireEvent.click(
      screen.getByRole("button", { name: zh.platformAuth.retryConfig }),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "登录智枢" })).toBeEnabled(),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("completes login through the browser authorization result", async () => {
    vi.mocked(platformAuthApi.loginOAuth).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    await submitOAuth();
    await screen.findByText("工具主页");
    expect(platformAuthApi.loginOAuth).toHaveBeenCalledWith(account.baseUrl);
  });

  it("shows failed authorization without opening the app", async () => {
    vi.mocked(platformAuthApi.loginOAuth).mockRejectedValue({
      code: "OAUTH_DENIED",
      message: "授权被拒绝",
    });
    renderGate();
    await submitOAuth();
    expect(await screen.findByRole("alert")).toHaveTextContent("授权被拒绝");
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录智枢" })).toBeEnabled();
  });

  it("cancels an in-flight authorization and blocks server edits until it finishes", async () => {
    let rejectLogin!: (reason: unknown) => void;
    vi.mocked(platformAuthApi.loginOAuth).mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectLogin = reject;
        }),
    );
    vi.mocked(platformAuthApi.cancelLogin).mockImplementation(async () => {
      rejectLogin({ code: "OAUTH_CANCELLED", message: "已取消授权登录" });
    });
    renderGate();
    await submitOAuth();
    expect(screen.getByLabelText(zh.platformAuth.address)).toBeDisabled();
    fireEvent.click(await screen.findByRole("button", { name: "取消授权" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "登录智枢" })).toBeEnabled(),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("discards a late authorization result after cancellation", async () => {
    let finish!: (value: {
      status: "authenticated";
      account: PlatformAccount;
    }) => void;
    vi.mocked(platformAuthApi.loginOAuth).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    renderGate();
    await submitOAuth();
    fireEvent.click(await screen.findByRole("button", { name: "取消授权" }));
    await act(async () => {
      finish({ status: "authenticated", account });
    });
    expect(screen.queryByText("工具主页")).not.toBeInTheDocument();
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
  });

  it("disables stale authorization config while a server address changes", async () => {
    vi.mocked(platformAuthApi.loginOAuth).mockResolvedValue({
      status: "authenticated",
      account,
    });
    renderGate();
    await screen.findByRole("button", { name: "登录智枢" });
    const address = screen.getByLabelText(zh.platformAuth.address);
    fireEvent.change(address, { target: { value: "https://other.test" } });
    expect(screen.getByRole("button", { name: "登录智枢" })).toBeDisabled();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "登录智枢" })).toBeEnabled(),
    );
    await submitOAuth();
    expect(platformAuthApi.loginOAuth).toHaveBeenCalledWith(
      "https://other.test",
    );
  });

  it("shows a persistence warning without rejecting a successful login", async () => {
    vi.mocked(platformAuthApi.loginOAuth).mockResolvedValue({
      status: "authenticated",
      account,
      persistenceWarning: "无法保存登录会话",
    });
    renderGate();
    await submitOAuth();
    await screen.findByText("工具主页");
    expect(toast.warning).toHaveBeenCalledWith("无法保存登录会话");
  });

  it("clears the UI on logout even when server revocation fails", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logout).mockRejectedValue(new Error("offline"));
    renderGate();
    fireEvent.click(
      await screen.findByRole("button", { name: "退出平台登录" }),
    );
    await screen.findByRole("button", { name: "登录智枢" });
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
    await screen.findByRole("button", { name: "登录智枢" });
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
    expect(platformAuthApi.loginOAuth).not.toHaveBeenCalled();
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
    expect(platformAuthApi.loginOAuth).not.toHaveBeenCalled();
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
    fireEvent.click(screen.getByRole("button", { name: zh.platformAuth.back }));
    await screen.findByRole("button", { name: "登录智枢" });
    expect(platformAuthApi.logout).toHaveBeenCalledOnce();
    expect(platformAuthApi.loginOAuth).not.toHaveBeenCalled();
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
    await screen.findByRole("button", { name: "登录智枢" });
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
    await screen.findByRole("button", { name: "登录智枢" });
  });
});

describe("catalog sync after verified login", () => {
  it("does not attempt provider sync when catalog sync fails", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformModelsApi.syncCatalog).mockRejectedValue({
      code: "NETWORK",
      message: "目录暂时不可用",
    });
    renderGate();
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("目录暂时不可用"),
    );
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    expect(screen.getByText("工具主页")).toBeInTheDocument();
    expect(platformAuthApi.logout).not.toHaveBeenCalled();
  });

  it("does not write provider lists on restoration", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    renderGate();
    await waitFor(() =>
      expect(platformModelsApi.syncCatalog).toHaveBeenCalledOnce(),
    );
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    expect(toast.warning).not.toHaveBeenCalled();
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
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
  });
  it("does not sync before browser authorization completes", async () => {
    let finish!: (value: {
      status: "authenticated";
      account: PlatformAccount;
    }) => void;
    vi.mocked(platformAuthApi.loginOAuth).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    renderGate();
    await submitOAuth();
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    await act(async () => finish({ status: "authenticated", account }));
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
    vi.mocked(platformAuthApi.loginOAuth).mockResolvedValue({
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
    await submitOAuth();
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

  it("shows a dismissible result dialog and returns to OAuth login", async () => {
    vi.mocked(platformAuthApi.account).mockResolvedValue(account);
    vi.mocked(platformAuthApi.logoutAndClear).mockResolvedValue(cleanupReport);
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
    expect(
      screen.getByRole("button", { name: "登录智枢", hidden: true }),
    ).toBeInTheDocument();
    expect(client.getQueryData(["providers", "claude"])).toBeUndefined();
    const resultDialog = screen.getByRole("dialog", {
      name: zh.platformCleanup.result,
    });
    expect(
      within(resultDialog).getByText(zh.platformCleanup.complete),
    ).toBeInTheDocument();
    expect(
      screen
        .getByRole("button", { name: "登录智枢", hidden: true })
        .closest("main"),
    ).not.toContainElement(resultDialog);
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
    expect(screen.queryByLabelText("密码")).not.toBeInTheDocument();
    expect(screen.getByLabelText(zh.platformAuth.address)).toHaveValue(
      account.baseUrl,
    );
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
    await screen.findByRole("button", { name: "登录智枢", hidden: true });
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
    await screen.findByRole("button", { name: "登录智枢", hidden: true });
    expect(platformAuthApi.logout).toHaveBeenCalledOnce();
    expect(platformAuthApi.logoutAndClear).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText(zh.platformCleanup.result),
    ).not.toBeInTheDocument();
  });
});
