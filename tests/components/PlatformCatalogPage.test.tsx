import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlatformCatalogPage } from "@/components/platform/PlatformCatalogPage";
import {
  platformModelsApi,
  type PlatformCatalog,
} from "@/lib/api/platformModels";
import { platformCatalogKeys } from "@/lib/query/platformCatalog";
import { toast } from "@/lib/toast";
import { copyText } from "@/lib/clipboard";

const auth = vi.hoisted(() => ({
  account: {
    baseUrl: "https://platform.test",
    user: { id: 1, email: "one@test.com" },
  },
}));
vi.mock("@/contexts/PlatformAuthContext", () => ({
  usePlatformAuth: () => auth,
}));
vi.mock("@/lib/api/platformModels", () => ({
  platformModelsApi: {
    catalog: vi.fn(),
    catalogKey: vi.fn(),
    syncCatalog: vi.fn(),
    sync: vi.fn(),
  },
}));
vi.mock("@/lib/clipboard", () => ({ copyText: vi.fn() }));
vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));

const snapshot: PlatformCatalog = {
  baseUrl: "https://platform.test",
  userId: 1,
  syncedAt: 1700000000000,
  groups: [
    {
      id: 42,
      name: "Shared group",
      platform: "anthropic",
      subscriptionType: "standard",
      status: "active",
      keyId: 123,
      keyName: "zhishu-ccs-anthropic-42",
      failed: false,
      error: null,
      models: [{ id: "claude-test", name: "Test model" }],
    },
  ],
  warnings: [],
};

function renderPage() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const view = render(
    <QueryClientProvider client={client}>
      <PlatformCatalogPage />
    </QueryClientProvider>,
  );
  return { client, invalidate, ...view };
}

beforeEach(() => {
  vi.clearAllMocks();
  auth.account = {
    baseUrl: "https://platform.test",
    user: { id: 1, email: "one@test.com" },
  };
  vi.mocked(platformModelsApi.catalog).mockReset().mockResolvedValue(null);
  vi.mocked(platformModelsApi.catalogKey)
    .mockReset()
    .mockResolvedValue("test-only-group-secret");
  vi.mocked(copyText).mockReset().mockResolvedValue(undefined);
  vi.mocked(platformModelsApi.syncCatalog)
    .mockReset()
    .mockResolvedValue(snapshot);
});

describe("shared platform base data", () => {
  it("syncs all apps from local data without fetching platform data or usage", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    vi.mocked(platformModelsApi.sync).mockResolvedValue({
      added: 2,
      removed: 0,
      skipped: 0,
      warnings: [],
    });
    const { invalidate } = renderPage();
    await screen.findByText("Shared group");
    fireEvent.click(screen.getByRole("button", { name: "全部同步平台供应商" }));
    await waitFor(() =>
      expect(platformModelsApi.sync).toHaveBeenCalledWith(undefined),
    );
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["providers"] });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["configChangeHistory"],
    });
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: ["platformUsage"],
    });
  });

  it("disables all-app sync until local credentials are available", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue({
      ...snapshot,
      credentialsCleared: true,
    });
    renderPage();
    await screen.findByText("Shared group");
    expect(
      screen.getByRole("button", { name: "全部同步平台供应商" }),
    ).toBeDisabled();
  });

  it("reads saved data on entry and only contacts the platform after a manual click", async () => {
    const { client, invalidate } = renderPage();
    await screen.findByText("尚无当前账号的基础数据，请先点击上方按钮同步。");
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(screen.getByText("Shared group")).toBeInTheDocument();
    expect(
      client.getQueryData(platformCatalogKeys.account(snapshot.baseUrl, 1)),
    ).toEqual(snapshot);
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    expect(invalidate).not.toHaveBeenCalled();
  });

  it("keeps the saved data visible after a failed platform update and allows retry", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    vi.mocked(platformModelsApi.syncCatalog).mockRejectedValueOnce(
      new Error("secret-token"),
    );
    renderPage();
    await screen.findByText("Shared group");
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await screen.findByRole("alert");
    expect(screen.getByText("Shared group")).toBeInTheDocument();
    expect(screen.queryByText("secret-token")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
  });

  it("shows partial group failures without reporting full success", async () => {
    vi.mocked(platformModelsApi.syncCatalog).mockResolvedValue({
      ...snapshot,
      groups: [
        {
          ...snapshot.groups[0],
          failed: true,
          error: "目录暂时不可用",
          models: [],
        },
      ],
    });
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await waitFor(() => expect(toast.warning).toHaveBeenCalled());
    expect(screen.getByText("同步失败，可重试")).toBeInTheDocument();
    expect(screen.getByText("目录暂时不可用")).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("disables repeated clicks while the platform request is pending", async () => {
    let finish!: (value: PlatformCatalog) => void;
    vi.mocked(platformModelsApi.syncCatalog).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    const button = await screen.findByRole("button", {
      name: "正在同步平台数据…",
    });
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(platformModelsApi.syncCatalog).toHaveBeenCalledTimes(1);
    finish(snapshot);
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
  });

  it("does not restore cleared account data when a platform sync completes after logout", async () => {
    let finish!: (value: PlatformCatalog) => void;
    vi.mocked(platformModelsApi.syncCatalog).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { client, unmount } = renderPage();
    await screen.findByText("尚无当前账号的基础数据，请先点击上方按钮同步。");
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await screen.findByRole("button", { name: "正在同步平台数据…" });
    await act(async () => {
      unmount();
      client.clear();
      finish(snapshot);
    });
    expect(
      client.getQueryData(platformCatalogKeys.account(snapshot.baseUrl, 1)),
    ).toBeUndefined();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("does not show another account's cached catalog", async () => {
    auth.account = { ...auth.account, user: { id: 2, email: "two@test.com" } };
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    renderPage();
    await screen.findByText("尚无当前账号的基础数据，请先点击上方按钮同步。");
    expect(screen.queryByText("Test model")).not.toBeInTheDocument();
  });

  it("allows a platform sync to replace an unreadable local snapshot", async () => {
    vi.mocked(platformModelsApi.catalog).mockRejectedValue({
      code: "CATALOG_STORE",
      message: "基础数据无法读取",
    });
    renderPage();
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await screen.findByText("Shared group");
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
  });

  it("opens local group and model details in the right drawer without syncing", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue({
      ...snapshot,
      groups: [
        {
          ...snapshot.groups[0],
          models: [
            {
              ...snapshot.groups[0].models[0],
              contextWindow: 128000,
              maxOutputTokens: 4096,
              endpoints: ["anthropic"],
              supportsTools: true,
              supportsStreaming: false,
              inputModalities: ["text", "image"],
              reasoningLevels: ["low", "high"],
              defaultReasoningLevel: "high",
            },
          ],
        },
      ],
    });
    renderPage();
    const user = userEvent.setup();
    const trigger = await screen.findByRole("button", {
      name: "查看 Shared group 的详情",
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(trigger);
    const drawer = await screen.findByRole("dialog", { name: "Shared group" });
    expect(within(drawer).getByText("42")).toBeInTheDocument();
    expect(within(drawer).getByText("按量计费")).toBeInTheDocument();
    expect(within(drawer).getByText("123")).toBeInTheDocument();
    expect(
      within(drawer).getByText("zhishu-ccs-anthropic-42"),
    ).toBeInTheDocument();
    expect(within(drawer).queryByText("Test model")).not.toBeInTheDocument();
    const expandCatalog = within(drawer).getByRole("button", {
      name: "展开模型目录",
    });
    expect(expandCatalog).toHaveAttribute("aria-expanded", "false");
    await user.click(expandCatalog);
    expect(within(drawer).getByText("Test model")).toBeInTheDocument();
    expect(within(drawer).getByText("claude-test")).toBeInTheDocument();
    expect(within(drawer).queryByText("128,000")).not.toBeInTheDocument();
    const expandModel = within(drawer).getByRole("button", {
      name: "Test model 的模型参数",
    });
    expect(expandModel).toHaveAttribute("aria-expanded", "false");
    await user.click(expandModel);
    expect(within(drawer).getByText("128,000")).toBeInTheDocument();
    expect(within(drawer).getByText("4,096")).toBeInTheDocument();
    expect(within(drawer).getByText("支持")).toBeInTheDocument();
    expect(within(drawer).getByText("不支持")).toBeInTheDocument();
    expect(within(drawer).getByText("low · high")).toBeInTheDocument();
    expect(platformModelsApi.catalog).toHaveBeenCalledTimes(1);
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
    expect(platformModelsApi.sync).not.toHaveBeenCalled();
    expect(platformModelsApi.catalogKey).not.toHaveBeenCalled();
    await user.click(expandModel);
    expect(within(drawer).queryByText("128,000")).not.toBeInTheDocument();
    await user.click(
      within(drawer).getByRole("button", { name: "收起模型目录" }),
    );
    expect(within(drawer).queryByText("Test model")).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(trigger).toHaveFocus();
  });

  it("shows unavailable parameters as unreported and supports closing the drawer", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    renderPage();
    const trigger = await screen.findByRole("button", {
      name: "查看 Shared group 的详情",
    });
    fireEvent.click(trigger);
    const drawer = await screen.findByRole("dialog");
    fireEvent.click(
      within(drawer).getByRole("button", { name: "展开模型目录" }),
    );
    fireEvent.click(
      within(drawer).getByRole("button", { name: "Test model 的模型参数" }),
    );
    expect(within(drawer).getAllByText("未提供").length).toBeGreaterThan(0);
    expect(within(drawer).queryByText("支持")).not.toBeInTheDocument();
    fireEvent.click(within(drawer).getByRole("button", { name: "关闭" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });

  it("reveals keys only on request, copies them and masks them again after reopening", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    const { client } = renderPage();
    const user = userEvent.setup();
    const trigger = await screen.findByRole("button", {
      name: "查看 Shared group 的详情",
    });
    await user.click(trigger);
    let drawer = await screen.findByRole("dialog");
    expect(platformModelsApi.catalogKey).not.toHaveBeenCalled();
    expect(
      within(drawer).queryByText("test-only-group-secret"),
    ).not.toBeInTheDocument();
    await user.click(
      within(drawer).getByRole("button", { name: "显示 API Key" }),
    );
    await within(drawer).findByText("test-only-group-secret");
    expect(platformModelsApi.catalogKey).toHaveBeenCalledWith(
      snapshot.baseUrl,
      snapshot.userId,
      42,
    );
    expect(
      JSON.stringify(
        client.getQueryData(platformCatalogKeys.account(snapshot.baseUrl, 1)),
      ),
    ).not.toContain("test-only-group-secret");
    await user.click(
      within(drawer).getByRole("button", { name: "复制 API Key" }),
    );
    await waitFor(() =>
      expect(copyText).toHaveBeenCalledWith("test-only-group-secret"),
    );
    await user.click(
      within(drawer).getByRole("button", { name: "隐藏 API Key" }),
    );
    expect(
      within(drawer).queryByText("test-only-group-secret"),
    ).not.toBeInTheDocument();
    await user.click(
      within(drawer).getByRole("button", { name: "显示 API Key" }),
    );
    await within(drawer).findByText("test-only-group-secret");
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(trigger);
    drawer = await screen.findByRole("dialog");
    expect(
      within(drawer).queryByText("test-only-group-secret"),
    ).not.toBeInTheDocument();
    expect(
      within(drawer).getByRole("button", { name: "显示 API Key" }),
    ).toBeInTheDocument();
    expect(platformModelsApi.syncCatalog).not.toHaveBeenCalled();
  });

  it("keeps logged-out account metadata visible and requires a platform sync to restore keys", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue({
      ...snapshot,
      credentialsCleared: true,
      groups: [{ ...snapshot.groups[0], keyAvailable: false }],
    });
    renderPage();
    await screen.findByText("Shared group");
    expect(screen.getByRole("status")).toHaveTextContent(
      "基础数据已保留，凭证已清除",
    );
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "查看 Shared group 的详情" }),
    );
    const drawer = await screen.findByRole("dialog");
    expect(
      within(drawer).getByText("zhishu-ccs-anthropic-42"),
    ).toBeInTheDocument();
    expect(
      within(drawer).queryByRole("button", { name: "显示 API Key" }),
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole("button", { name: "复制 API Key" }),
    ).not.toBeInTheDocument();
    await user.click(
      within(drawer).getByRole("button", { name: "展开模型目录" }),
    );
    expect(within(drawer).getByText("Test model")).toBeInTheDocument();
    expect(platformModelsApi.catalogKey).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "同步平台基础数据" }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(
      screen.queryByText(
        "基础数据已保留，凭证已清除。同步平台基础数据后可恢复使用。",
      ),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "查看 Shared group 的详情" }),
    );
    expect(
      within(await screen.findByRole("dialog")).getByRole("button", {
        name: "显示 API Key",
      }),
    ).toBeEnabled();
  });

  it("copies a masked key without revealing it and handles unavailable keys", async () => {
    vi.mocked(platformModelsApi.catalog).mockResolvedValue(snapshot);
    renderPage();
    fireEvent.click(
      await screen.findByRole("button", { name: "查看 Shared group 的详情" }),
    );
    const drawer = await screen.findByRole("dialog");
    fireEvent.click(
      within(drawer).getByRole("button", { name: "复制 API Key" }),
    );
    await waitFor(() =>
      expect(copyText).toHaveBeenCalledWith("test-only-group-secret"),
    );
    expect(
      within(drawer).queryByText("test-only-group-secret"),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(drawer).getByRole("button", { name: "显示 API Key" }),
      ).toBeEnabled(),
    );
    vi.mocked(platformModelsApi.catalogKey).mockResolvedValue(null);
    fireEvent.click(
      within(drawer).getByRole("button", { name: "显示 API Key" }),
    );
    await waitFor(() => expect(toast.warning).toHaveBeenCalled());
  });
});
