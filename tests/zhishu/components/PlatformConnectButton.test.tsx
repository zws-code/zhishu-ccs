import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlatformConnectButton } from "@/zhishu/models/components/PlatformConnectButton";
import { platformModelsApi } from "@/zhishu/api/platformModels";
import { toast } from "@/lib/toast";
import type { AppId } from "@/lib/api/index";

const auth = vi.hoisted(() => ({
  signedIn: true,
  userId: 1,
  baseUrl: "https://platform.test",
}));
vi.mock("@/zhishu/auth/PlatformAuthContext", () => ({
  usePlatformAuth: () => ({
    account: auth.signedIn
      ? { baseUrl: auth.baseUrl, user: { id: auth.userId } }
      : null,
  }),
}));
vi.mock("@/zhishu/api/platformModels", () => ({
  platformModelsApi: { sync: vi.fn() },
}));
vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));

function renderButton(app: AppId = "claude") {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const ui = () => (
    <QueryClientProvider client={client}>
      <PlatformConnectButton app={app} />
    </QueryClientProvider>
  );
  const view = render(ui());
  return { ...view, invalidate, refreshAccount: () => view.rerender(ui()) };
}

beforeEach(() => {
  vi.clearAllMocks();
  auth.signedIn = true;
  auth.userId = 1;
  auth.baseUrl = "https://platform.test";
  vi.mocked(platformModelsApi.sync).mockReset().mockResolvedValue({
    added: 2,
    removed: 1,
    skipped: 3,
    warnings: [],
  });
});

describe("sync current platform service providers", () => {
  it("distinguishes provider updates from native file writes and refreshes operation history", async () => {
    vi.mocked(platformModelsApi.sync).mockResolvedValue({
      added: 1,
      removed: 0,
      skipped: 5,
      updated: 5,
      nativeFilesChanged: 0,
      warnings: [],
    });
    const { invalidate } = renderButton("workbuddy");
    fireEvent.click(screen.getByRole("button", { name: "同步平台供应商" }));
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        "供应商列表：新增 1，更新 5，删除 0；未修改原生配置文件。详情见配置修改记录。",
      ),
    );
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["configChangeHistory"],
    });
  });

  it("syncs with only the app identifier and refreshes the original provider page", async () => {
    const { invalidate } = renderButton();
    fireEvent.click(screen.getByRole("button", { name: "同步平台供应商" }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(platformModelsApi.sync).toHaveBeenCalledTimes(1);
    expect(platformModelsApi.sync).toHaveBeenCalledWith("claude");
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "claude"],
    });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["proxyTakeoverStatus"],
    });
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: ["platformUsage"],
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("syncs WorkBuddy from its provider page", async () => {
    const { invalidate } = renderButton("workbuddy");
    fireEvent.click(screen.getByRole("button", { name: "同步平台供应商" }));

    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(platformModelsApi.sync).toHaveBeenCalledWith("workbuddy");
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "workbuddy"],
    });
  });

  it("syncs Kimi Code from its provider page", async () => {
    renderButton("kimi");
    fireEvent.click(screen.getByRole("button", { name: "同步平台供应商" }));

    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(platformModelsApi.sync).toHaveBeenCalledWith("kimi");
  });

  it("blocks repeated clicks while synchronization is running", async () => {
    let finish!: (
      value: Awaited<ReturnType<typeof platformModelsApi.sync>>,
    ) => void;
    vi.mocked(platformModelsApi.sync).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    renderButton("codex");
    fireEvent.click(screen.getByRole("button"));
    const pending = await screen.findByRole("button", {
      name: "正在同步平台供应商…",
    });
    expect(pending).toBeDisabled();
    fireEvent.click(pending);
    expect(platformModelsApi.sync).toHaveBeenCalledTimes(1);
    finish({
      added: 1,
      removed: 0,
      skipped: 2,
      warnings: [],
    });
    await waitFor(() => expect(screen.getByRole("button")).toBeEnabled());
  });

  it("reports partial synchronization warnings", async () => {
    vi.mocked(platformModelsApi.sync).mockResolvedValue({
      added: 1,
      removed: 0,
      skipped: 2,
      warnings: ["某个分组同步失败"],
    });
    renderButton();
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(toast.warning).toHaveBeenCalledWith(
        "某个分组同步失败（新增 1，删除 0，跳过 2）",
      ),
    );
    expect(toast.success).not.toHaveBeenCalled();
  });

  it.each([false, true])(
    "ignores a late sync result after unmount (failure=%s)",
    async (fails) => {
      let finish!: () => void;
      vi.mocked(platformModelsApi.sync).mockReturnValueOnce(
        new Promise((resolve, reject) => {
          finish = () =>
            fails
              ? reject(new Error("old operation"))
              : resolve({
                  added: 1,
                  removed: 0,
                  skipped: 0,
                  warnings: [],
                });
        }),
      );
      const view = renderButton();
      fireEvent.click(screen.getByRole("button"));
      await waitFor(() =>
        expect(platformModelsApi.sync).toHaveBeenCalledTimes(1),
      );
      view.unmount();
      await act(async () => finish());
      expect(view.invalidate).not.toHaveBeenCalled();
      expect(toast.success).not.toHaveBeenCalled();
      expect(toast.error).not.toHaveBeenCalled();
    },
  );

  it("refreshes the actual Claude Desktop status query after synchronization", async () => {
    const { invalidate } = renderButton("claude-desktop");
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["claudeDesktopStatus"],
    });
  });

  it.each(["logout", "user", "origin"])(
    "ignores pending sync after an account %s change",
    async (change) => {
      let finish!: () => void;
      vi.mocked(platformModelsApi.sync).mockReturnValueOnce(
        new Promise((resolve) => {
          finish = () =>
            resolve({ added: 1, removed: 0, skipped: 0, warnings: [] });
        }),
      );
      const view = renderButton();
      fireEvent.click(screen.getByRole("button"));
      await waitFor(() =>
        expect(platformModelsApi.sync).toHaveBeenCalledTimes(1),
      );
      if (change === "logout") auth.signedIn = false;
      if (change === "user") auth.userId = 2;
      if (change === "origin") auth.baseUrl = "https://another.test";
      view.refreshAccount();
      // Returning to the same account must not revive the abandoned operation.
      auth.signedIn = true;
      auth.userId = 1;
      auth.baseUrl = "https://platform.test";
      view.refreshAccount();
      await act(async () => finish());
      expect(view.invalidate).not.toHaveBeenCalled();
      expect(toast.success).not.toHaveBeenCalled();
    },
  );

  it("allows retry without exposing arbitrary IPC details", async () => {
    vi.mocked(platformModelsApi.sync).mockRejectedValueOnce(
      new Error("private-key-detail"),
    );
    renderButton();
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("无法完成平台请求，请稍后重试"),
    );
    expect(screen.getByRole("button")).toBeEnabled();
    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
  });

  it("only offers account connection for signed-in users and supported apps", () => {
    const desktop = renderButton("claude-desktop");
    expect(screen.getByRole("button")).toBeInTheDocument();
    desktop.unmount();

    const workbuddy = renderButton("workbuddy");
    expect(screen.getByRole("button")).toBeInTheDocument();
    workbuddy.unmount();

    const unsupported = renderButton("agy");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    unsupported.unmount();

    auth.signedIn = false;
    renderButton();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
