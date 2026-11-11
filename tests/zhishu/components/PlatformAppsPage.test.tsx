import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PlatformAppsPage } from "@/zhishu/models/components/PlatformAppsPage";
import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/zhishu/api/platformModels";
import { platformCatalogKeys } from "@/zhishu/models/query/platformCatalogKeys";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { settingsApi } from "@/lib/api/index";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
import { APP_IDS } from "@/config/appConfig";
import type { Provider } from "@/types";
import { toast } from "@/lib/toast";
import zh from "@/i18n/locales/zh.json";

const account = {
  baseUrl: "https://platform.test",
  user: {
    id: 1,
    email: "user@example.com",
    username: "测试账户",
    role: "user",
    balance: 10,
  },
};

vi.mock("@/zhishu/auth/PlatformAuthContext", () => ({
  usePlatformAuth: () => ({ account, catalogSyncing: false }),
}));
vi.mock("@/zhishu/api/platformModels", () => ({
  platformModelsApi: {
    sync: vi.fn(),
    catalog: vi.fn(),
    groupOptions: vi.fn(),
    deleteProviders: vi.fn(),
  },
}));

const settingsState = vi.hoisted(() => ({
  data: {
    visibleApps: {} as Record<string, boolean>,
    appOrder: ["claude", "codex", "workbuddy"],
  },
}));
const providersState = vi.hoisted(() => ({
  byApp: {} as Record<
    string,
    { providers: Record<string, Provider>; currentProviderId: string }
  >,
  /** 模拟后端：sync 之后该应用才出现平台托管行 */
  claudeProvisioned: false,
}));
vi.mock("@/lib/query/queries", () => ({
  useSettingsQuery: () => ({ data: settingsState.data }),
  providersQueryOptions: (appId: string) => ({
    queryKey: ["providers", appId],
    queryFn: () =>
      providersState.byApp[appId] ?? { providers: {}, currentProviderId: "" },
  }),
}));
vi.mock("@/lib/api/index", () => ({
  settingsApi: { listToolInstallations: vi.fn() },
}));
vi.mock("@/lib/api/providers", () => ({
  providersApi: {
    getAll: vi.fn(),
    getCurrent: vi.fn(),
    switch: vi.fn(),
    addToLiveConfig: vi.fn(),
    updateTrayMenu: vi.fn(),
    getClaudeDesktopStatus: vi.fn(),
    getWorkBuddyInstalled: vi.fn(),
  },
}));
vi.mock("@/lib/api/proxy", () => ({
  proxyApi: {
    getAppMode: vi.fn(),
    setProxyTakeoverForApp: vi.fn(),
    setProxyStackMember: vi.fn(),
  },
}));
vi.mock("@/zhishu/integrations/ccswitch/platformProviders", () => ({
  refreshPlatformProviders: vi.fn(),
}));
vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
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

const catalog: PlatformCatalog = {
  baseUrl: account.baseUrl,
  userId: account.user.id,
  syncedAt: 1,
  groups: [42, 43].map((id, index) => ({
    id,
    name: index === 0 ? "Shared group" : "Second group",
    platform: "anthropic",
    subscriptionType: "standard",
    status: "active",
    keyId: id,
    keyAvailable: true,
    quota: { available: true, reasons: [] },
    subscriptions: [],
    failed: false,
    error: null,
    models: [{ id: `claude-test-${id}`, name: "Test model" }],
  })),
  warnings: [],
};

const managedProvider = (groupId: number, id: string) => ({
  id,
  name: groupId === 42 ? "智枢平台 · Shared group" : "智枢平台 · Second group",
  settingsConfig: {},
  meta: {
    platformManaged: {
      origin: account.baseUrl,
      userId: account.user.id,
      entryId: `entry-${groupId}`,
      model: `claude-test-${groupId}`,
      keyId: groupId,
      groupId,
    },
  },
});

const selfBuiltProvider = {
  id: "self-built",
  name: "自建供应商",
  settingsConfig: {},
};

const claudeManaged = managedProvider(42, "platform-42");

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  client.setQueryData(
    platformCatalogKeys.account(account.baseUrl, account.user.id),
    catalog,
  );
  render(
    <QueryClientProvider client={client}>
      <PlatformAppsPage onOpenApp={vi.fn()} />
    </QueryClientProvider>,
  );
  return client;
}

beforeEach(() => {
  window.localStorage.clear();
  providersState.byApp = {};
  providersState.claudeProvisioned = false;
  // jsdom 没有 scrollIntoView，下拉打开时会调用。
  Element.prototype.scrollIntoView = vi.fn();
  vi.clearAllMocks();
  settingsState.data = {
    // 与 App/向导一致的合并方式：显式关掉默认可见应用，只留本用例的三个
    visibleApps: {
      ...Object.fromEntries(APP_IDS.map((app) => [app, false])),
      claude: true,
      codex: true,
      workbuddy: true,
    },
    appOrder: ["claude", "codex", "workbuddy"],
  };
  vi.mocked(platformModelsApi.catalog).mockResolvedValue(catalog);
  vi.mocked(platformModelsApi.groupOptions).mockImplementation(async (app) =>
    app === "claude"
      ? [
          {
            groupId: 42,
            groupName: "Shared group",
            platform: "anthropic",
            modelCount: 1,
          },
          {
            groupId: 43,
            groupName: "Second group",
            platform: "anthropic",
            modelCount: 1,
          },
        ]
      : [],
  );
  vi.mocked(platformModelsApi.sync).mockImplementation(async () => {
    // 平台托管行由同步命令落库，之后 getAll 才读得到
    providersState.claudeProvisioned = true;
    return { added: 2, removed: 0, skipped: 0, warnings: [] };
  });
  vi.mocked(platformModelsApi.deleteProviders).mockResolvedValue({
    removed: 1,
    skipped: 0,
    warnings: [],
    nativeFilesChanged: 0,
  });
  vi.mocked(settingsApi.listToolInstallations).mockResolvedValue([
    { tool: "claude", installs: [{}] },
    { tool: "codex", installs: [{}] },
  ] as never);
  vi.mocked(providersApi.getAll).mockImplementation(async (app) => {
    if (app === "claude" && providersState.claudeProvisioned) {
      return { [claudeManaged.id]: claudeManaged };
    }
    return providersState.byApp[app]?.providers ?? {};
  });
  vi.mocked(providersApi.getCurrent).mockImplementation(
    async (app) => providersState.byApp[app]?.currentProviderId ?? "",
  );
  vi.mocked(providersApi.switch).mockResolvedValue({ warnings: [] });
  vi.mocked(proxyApi.getAppMode).mockResolvedValue({
    mode: "direct",
    attached: false,
    routeProviderId: null,
    directProviderId: null,
  });
  vi.mocked(proxyApi.setProxyTakeoverForApp).mockResolvedValue(undefined);
  vi.mocked(proxyApi.setProxyStackMember).mockResolvedValue(null);
  vi.mocked(providersApi.updateTrayMenu).mockResolvedValue(true);
  vi.mocked(providersApi.getClaudeDesktopStatus).mockResolvedValue({
    installed: false,
  } as never);
  vi.mocked(providersApi.getWorkBuddyInstalled).mockResolvedValue(false);
  vi.mocked(refreshPlatformProviders).mockResolvedValue(undefined);
});

describe("platform apps page", () => {
  it("shows each app's toggle state from the current account's managed providers", async () => {
    const p1 = managedProvider(42, "platform-42");
    providersState.byApp["claude"] = {
      providers: {
        [p1.id]: p1,
        [selfBuiltProvider.id]: selfBuiltProvider,
        "other-account": {
          id: "other-account",
          name: "其他账号",
          settingsConfig: {},
          meta: {
            platformManaged: { ...p1.meta.platformManaged, userId: 999 },
          },
        },
      },
      currentProviderId: p1.id,
    };
    renderPage();

    // 当前生效的是本账号托管行：开 + 已接入标签 + 供应商名,模型
    const claudeSwitch = await screen.findByRole("switch", {
      name: "移除 Claude Code 的平台供应商",
    });
    expect(claudeSwitch).toBeChecked();
    expect(screen.getByText("已接入")).toBeInTheDocument();
    expect(
      screen.getByText("智枢平台 · Shared group,claude-test-42"),
    ).toBeInTheDocument();
    // 自建和其他账号的行不参与开关状态展示
    expect(screen.queryByText("自建供应商")).not.toBeInTheDocument();

    // 已安装但未接入：关
    const codexSwitch = screen.getByRole("switch", {
      name: "接入 Codex 的平台供应商",
    });
    expect(codexSwitch).not.toBeChecked();
    expect(screen.getByText("未接入")).toBeInTheDocument();

    // 未安装：禁用 + 未安装文案
    const workbuddySwitch = screen.getByRole("switch", {
      name: "接入 WorkBuddy 的平台供应商",
    });
    expect(workbuddySwitch).toBeDisabled();
    expect(screen.getByText("未安装")).toBeInTheDocument();
  });

  it("disables mode apps until group options are available", async () => {
    renderPage();

    // Codex 没有可用分组（目录里只有 Claude 的分组），开关先禁用
    await waitFor(() =>
      expect(
        screen.getByRole("switch", { name: "接入 Codex 的平台供应商" }),
      ).toBeDisabled(),
    );
  });

  it("connects through the wizard's shared provisioning logic when turned on", async () => {
    const user = userEvent.setup();
    const client = renderPage();

    const claudeSwitch = await screen.findByRole("switch", {
      name: "接入 Claude Code 的平台供应商",
    });
    await user.click(claudeSwitch);

    await waitFor(() =>
      expect(platformModelsApi.sync).toHaveBeenCalledWith("claude"),
    );
    await waitFor(() =>
      expect(providersApi.switch).toHaveBeenCalledWith(
        "platform-42",
        "claude",
      ),
    );
    expect(proxyApi.setProxyTakeoverForApp).toHaveBeenCalledWith(
      "claude",
      false,
    );
    expect(refreshPlatformProviders).toHaveBeenCalledWith(client, "claude");
    expect(toast.success).toHaveBeenCalledWith(
      "已写入直连配置：智枢平台 · Shared group",
    );
  });

  it("removes only the current account's managed providers when turned off", async () => {
    const user = userEvent.setup();
    const p1 = managedProvider(42, "platform-42");
    providersState.byApp["claude"] = {
      providers: { [p1.id]: p1, [selfBuiltProvider.id]: selfBuiltProvider },
      currentProviderId: p1.id,
    };
    renderPage();

    const claudeSwitch = await screen.findByRole("switch", {
      name: "移除 Claude Code 的平台供应商",
    });
    await user.click(claudeSwitch);

    await waitFor(() =>
      expect(platformModelsApi.deleteProviders).toHaveBeenCalledWith(
        "claude",
        account.baseUrl,
        account.user.id,
      ),
    );
    // 关闭走删除平台托管行的既有逻辑，不触碰其他供应商行
    expect(providersApi.switch).not.toHaveBeenCalled();
    expect(toast.success).toHaveBeenCalled();
  });

  it("prefills the recommended group and mode defaults", async () => {
    const p1 = managedProvider(42, "platform-42");
    const p2 = managedProvider(43, "platform-43");
    providersState.byApp["claude"] = {
      providers: { [p1.id]: p1, [p2.id]: p2 },
      currentProviderId: p2.id,
    };
    vi.mocked(proxyApi.getAppMode).mockResolvedValue({
      mode: "route",
      attached: true,
      routeProviderId: p2.id,
      directProviderId: null,
    });
    renderPage();

    // 当前生效的是路由里的 Second group：分组沿用 43，模式沿用路由
    await waitFor(() =>
      expect(screen.getByText("Second group（1 个模型）")).toBeInTheDocument(),
    );
    const modeSelect = screen.getByRole("combobox", {
      name: "Claude Code 的接入方式",
    });
    expect(modeSelect).toHaveTextContent("路由");
  });
});
