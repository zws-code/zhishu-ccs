import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PlatformOnboardingProvider } from "@/zhishu/onboarding/PlatformOnboarding";
import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/zhishu/api/platformModels";
import { platformCatalogKeys } from "@/zhishu/models/query/platformCatalogKeys";
import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { APP_IDS } from "@/config/appConfig";
import { settingsApi } from "@/lib/api/index";
import { refreshPlatformProviders } from "@/zhishu/integrations/ccswitch/platformProviders";
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
  usePlatformAuth: () => ({ account }),
}));
vi.mock("@/zhishu/api/platformModels", () => ({
  platformModelsApi: {
    sync: vi.fn(),
    catalog: vi.fn(),
    groupOptions: vi.fn(),
  },
}));
const settingsState = vi.hoisted(() => ({
  visibleApps: { claude: true, codex: true } as Record<string, boolean>,
  appOrder: ["claude", "codex"],
}));
vi.mock("@/lib/query/queries", () => ({
  useSettingsQuery: () => ({
    data: {
      visibleApps: settingsState.visibleApps,
      appOrder: settingsState.appOrder,
    },
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
  groups: [
    {
      id: 42,
      name: "Shared group",
      platform: "anthropic",
      subscriptionType: "standard",
      status: "active",
      keyId: 123,
      keyName: "zhishu-client-42",
      keyAvailable: true,
      quota: { available: true, reasons: [] },
      subscriptions: [],
      failed: false,
      error: null,
      models: [{ id: "claude-test", name: "Test model" }],
    },
  ],
  warnings: [],
};

const managedProvider = {
  id: "platform-managed-claude-scope-anthropic-42",
  name: "智枢平台 · Shared group",
  settingsConfig: {},
  meta: {
    platformManaged: {
      origin: account.baseUrl,
      userId: account.user.id,
      entryId: "entry",
      model: "claude-test",
      keyId: 123,
      groupId: 42,
    },
  },
};

const secondManagedProvider = {
  id: "platform-managed-claude-scope-anthropic-43",
  name: "智枢平台 · Second group",
  settingsConfig: {},
  meta: {
    platformManaged: {
      origin: account.baseUrl,
      userId: account.user.id,
      entryId: "entry-b",
      model: "claude-test-b",
      keyId: 124,
      groupId: 43,
    },
  },
};

const opencodeProviders = [42, 43].map((groupId) => ({
  id: `platform-managed-opencode-scope-anthropic-${groupId}`,
  name: `智枢平台 · OpenCode group ${groupId}`,
  settingsConfig: {},
  meta: {
    platformManaged: {
      origin: account.baseUrl,
      userId: account.user.id,
      entryId: `opencode-${groupId}`,
      model: "opencode-test",
      keyId: groupId,
      groupId,
    },
  },
}));

function renderOnboarding() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  client.setQueryData(
    platformCatalogKeys.account(account.baseUrl, account.user.id),
    catalog,
  );
  client.setQueryData(["settings"], {
    firstRunNoticeConfirmed: true,
    visibleApps: { claude: true, codex: true },
    appOrder: ["claude", "codex"],
  });
  render(
    <QueryClientProvider client={client}>
      <PlatformOnboardingProvider>
        <p>工具主页</p>
      </PlatformOnboardingProvider>
    </QueryClientProvider>,
  );
  return client;
}

beforeEach(() => {
  window.localStorage.clear();
  settingsState.visibleApps = { claude: true, codex: true };
  settingsState.appOrder = ["claude", "codex"];
  // jsdom 没有 scrollIntoView，下拉打开时会调用。
  Element.prototype.scrollIntoView = vi.fn();
  vi.clearAllMocks();
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
  vi.mocked(platformModelsApi.sync).mockResolvedValue({
    added: 1,
    removed: 0,
    skipped: 0,
    warnings: [],
  });
  vi.mocked(settingsApi.listToolInstallations).mockResolvedValue([
    { tool: "claude", installs: [{}] },
  ] as never);
  vi.mocked(providersApi.getAll).mockImplementation(async (app) =>
    app === "claude" ? { [managedProvider.id]: managedProvider } : {},
  );
  vi.mocked(providersApi.getCurrent).mockResolvedValue("");
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
  vi.mocked(providersApi.getClaudeDesktopStatus).mockResolvedValue({} as never);
  vi.mocked(providersApi.getWorkBuddyInstalled).mockResolvedValue(false);
  vi.mocked(refreshPlatformProviders).mockResolvedValue(undefined);
});

describe("platform onboarding", () => {
  it("opens once and connects the installed app after confirmation", async () => {
    renderOnboarding();

    expect(await screen.findByText("接入工具")).toBeInTheDocument();
    // 已安装的 Claude 默认勾选，未安装的 Codex 不勾选
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: "Claude Code" }),
      ).toBeChecked(),
    );
    expect(screen.getByRole("checkbox", { name: "Codex" })).not.toBeChecked();

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    await waitFor(() =>
      expect(platformModelsApi.sync).toHaveBeenCalledWith("claude"),
    );
    expect(platformModelsApi.sync).not.toHaveBeenCalledWith("codex");
    expect(proxyApi.setProxyTakeoverForApp).toHaveBeenCalledWith(
      "claude",
      false,
    );
    expect(providersApi.switch).toHaveBeenCalledWith(
      managedProvider.id,
      "claude",
    );
    expect(
      await screen.findByText("已写入直连配置：智枢平台 · Shared group"),
    ).toBeInTheDocument();
    expect(toast.success).toHaveBeenCalled();
    expect(
      window.localStorage.getItem(
        `zhishu-platform-onboarding-dismissed:${encodeURIComponent(account.baseUrl)}:${account.user.id}`,
      ),
    ).toBe("1");
  });

  it("connects the selected group instead of the first one", async () => {
    const user = userEvent.setup();
    vi.mocked(providersApi.getAll).mockResolvedValue({
      [managedProvider.id]: managedProvider,
      [secondManagedProvider.id]: secondManagedProvider,
    });
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: "Claude Code" }),
      ).toBeChecked(),
    );
    const groupSelect = await screen.findByRole("combobox", {
      name: "Claude Code 的分组",
    });
    await waitFor(() => expect(groupSelect).toBeEnabled());
    await user.click(groupSelect);
    await user.click(
      await screen.findByRole("option", { name: "Second group（1 个模型）" }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    await waitFor(() =>
      expect(providersApi.switch).toHaveBeenCalledWith(
        secondManagedProvider.id,
        "claude",
      ),
    );
    expect(
      await screen.findByText("已写入直连配置：智枢平台 · Second group"),
    ).toBeInTheDocument();
  });

  it("does not auto-open after the account dismissed it", async () => {
    window.localStorage.setItem(
      `zhishu-platform-onboarding-dismissed:${encodeURIComponent(account.baseUrl)}:${account.user.id}`,
      "1",
    );
    renderOnboarding();

    await waitFor(() =>
      expect(screen.getByText("工具主页")).toBeInTheDocument(),
    );
    expect(screen.queryByText("接入工具")).not.toBeInTheDocument();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
  });

  it("connects a mode app through the selected route mode", async () => {
    const user = userEvent.setup();
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: "Claude Code" }),
      ).toBeChecked(),
    );
    await user.click(
      screen.getByRole("combobox", { name: "Claude Code 的接入方式" }),
    );
    await user.click(await screen.findByRole("option", { name: "路由" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    await waitFor(() =>
      expect(proxyApi.setProxyTakeoverForApp).toHaveBeenCalledWith(
        "claude",
        true,
        false,
        managedProvider.id,
      ),
    );
    expect(providersApi.switch).not.toHaveBeenCalled();
    expect(
      await screen.findByText("已切换路由：智枢平台 · Shared group"),
    ).toBeInTheDocument();
  });

  it("adds every platform provider when stack mode is selected", async () => {
    const user = userEvent.setup();
    vi.mocked(providersApi.getAll).mockResolvedValue({
      [managedProvider.id]: managedProvider,
      [secondManagedProvider.id]: secondManagedProvider,
    });
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: "Claude Code" }),
      ).toBeChecked(),
    );
    await user.click(
      screen.getByRole("combobox", { name: "Claude Code 的接入方式" }),
    );
    await user.click(await screen.findByRole("option", { name: "聚合" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    await waitFor(() =>
      expect(proxyApi.setProxyTakeoverForApp).toHaveBeenCalledWith(
        "claude",
        true,
        true,
        managedProvider.id,
      ),
    );
    expect(proxyApi.setProxyStackMember).toHaveBeenCalledWith(
      "claude",
      managedProvider.id,
      true,
    );
    expect(proxyApi.setProxyStackMember).toHaveBeenCalledWith(
      "claude",
      secondManagedProvider.id,
      true,
    );
    expect(
      await screen.findByText(
        "已加入聚合：智枢平台 · Shared group（共 2 个平台供应商）",
      ),
    ).toBeInTheDocument();
  });

  it("detects an installed WorkBuddy desktop app", async () => {
    settingsState.visibleApps = { workbuddy: true };
    settingsState.appOrder = ["workbuddy"];
    vi.mocked(settingsApi.listToolInstallations).mockResolvedValue([] as never);
    vi.mocked(providersApi.getWorkBuddyInstalled).mockResolvedValue(true);
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(screen.getByRole("checkbox", { name: "WorkBuddy" })).toBeChecked(),
    );
    expect(screen.getByText("已安装")).toBeInTheDocument();
    expect(providersApi.getWorkBuddyInstalled).toHaveBeenCalledTimes(1);
  });

  it("adds every platform provider for an additive app", async () => {
    settingsState.visibleApps = Object.fromEntries(
      APP_IDS.map((app) => [app, app === "opencode"]),
    );
    settingsState.appOrder = ["opencode"];
    vi.mocked(settingsApi.listToolInstallations).mockResolvedValue([
      { tool: "opencode", installs: [{}] },
    ] as never);
    vi.mocked(providersApi.getAll).mockResolvedValue(
      Object.fromEntries(
        opencodeProviders.map((provider) => [provider.id, provider]),
      ),
    );
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(screen.getByRole("checkbox", { name: "OpenCode" })).toBeChecked(),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    await waitFor(() =>
      expect(providersApi.switch).toHaveBeenCalledWith(
        opencodeProviders[0].id,
        "opencode",
      ),
    );
    expect(providersApi.switch).toHaveBeenCalledWith(
      opencodeProviders[1].id,
      "opencode",
    );
    expect(
      await screen.findByText("已添加到供应商列表：2 个平台供应商"),
    ).toBeInTheDocument();
    expect(platformModelsApi.groupOptions).not.toHaveBeenCalled();
  });

  it("keeps going when one app has no compatible platform model", async () => {
    vi.mocked(providersApi.getAll).mockResolvedValue({});
    renderOnboarding();

    await screen.findByText("接入工具");
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: "Claude Code" }),
      ).toBeChecked(),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "接入选中的应用" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "接入选中的应用" }));

    expect(
      await screen.findByText("接入失败：没有可用于此应用的平台模型"),
    ).toBeInTheDocument();
    expect(toast.warning).toHaveBeenCalled();
  });
});
