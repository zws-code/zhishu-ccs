import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlatformBulkSyncButtons } from "@/zhishu/models/components/PlatformBulkSyncButtons";
import type { AppId } from "@/lib/api/index";
import type { AppMode, ProxyStack } from "@/types/proxy";
import type { Provider } from "@/types";
import { createTestQueryClient } from "../../utils/testQueryClient";

const auth = vi.hoisted(() => ({ signedIn: true }));
vi.mock("@/zhishu/auth/PlatformAuthContext", () => ({
  usePlatformAuth: () => ({
    account: auth.signedIn
      ? { baseUrl: "https://platform.example.com", user: { id: 1 } }
      : null,
  }),
}));

vi.mock("@/lib/api/providers", () => ({
  providersApi: {
    getOpenCodeLiveProviderIds: vi.fn(),
    switch: vi.fn(),
    addToLiveConfig: vi.fn(),
    removeFromLiveConfig: vi.fn(),
    updateTrayMenu: vi.fn(),
  },
}));

vi.mock("@/lib/api/proxy", () => ({
  proxyApi: {
    getProxyStack: vi.fn(),
    setProxyStackMember: vi.fn(),
  },
}));

vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

import { providersApi } from "@/lib/api/providers";
import { proxyApi } from "@/lib/api/proxy";
import { toast } from "@/lib/toast";

function provider(
  id: string,
  options: { platform?: boolean; userId?: number; liveConfigManaged?: boolean } = {},
): Provider {
  return {
    id,
    name: `Provider ${id}`,
    category: "custom",
    settingsConfig: {},
    meta: {
      ...(options.platform
        ? {
            platformManaged: {
              origin: "https://platform.example.com",
              userId: options.userId ?? 1,
              entryId: `entry-${id}`,
              model: "standard",
              keyId: 1,
            },
          }
        : {}),
      ...(options.liveConfigManaged !== undefined
        ? { liveConfigManaged: options.liveConfigManaged }
        : {}),
    },
  };
}

function renderButtons(
  app: AppId,
  modeView: AppMode | null,
  providers: Provider[],
) {
  return render(
    <QueryClientProvider client={createTestQueryClient()}>
      <PlatformBulkSyncButtons
        app={app}
        modeView={modeView}
        providers={providers}
      />
    </QueryClientProvider>,
  );
}

function stackWith(memberIds: string[]): ProxyStack {
  return {
    active: true,
    members: memberIds.map((providerId) => ({
      providerId,
      modelIds: [],
      route: false,
      memberModels: 0,
    })),
  };
}

/** 成员名单查询解析前按钮是禁用的，等可用后再点。 */
async function clickWhenEnabled(name: string) {
  const button = screen.getByRole("button", { name });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
}

beforeEach(() => {
  vi.clearAllMocks();
  auth.signedIn = true;
  vi.mocked(providersApi.getOpenCodeLiveProviderIds).mockResolvedValue([]);
  vi.mocked(providersApi.switch).mockResolvedValue({ warnings: [] });
  vi.mocked(providersApi.addToLiveConfig).mockResolvedValue(true);
  vi.mocked(providersApi.removeFromLiveConfig).mockResolvedValue(true);
  vi.mocked(providersApi.updateTrayMenu).mockResolvedValue(true);
  vi.mocked(proxyApi.getProxyStack).mockResolvedValue(stackWith([]));
  vi.mocked(proxyApi.setProxyStackMember).mockResolvedValue(null);
});

describe("PlatformBulkSyncButtons visibility", () => {
  it("shows nothing for mode apps outside the stack view", () => {
    const { container } = renderButtons(
      "codex",
      "direct",
      [provider("p1", { platform: true })],
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows nothing for mode apps without a stack mode", () => {
    const { container } = renderButtons(
      "gemini",
      null,
      [provider("p1", { platform: true })],
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows nothing without platform providers of the current account", () => {
    const { container } = renderButtons("opencode", null, [
      provider("p4", { platform: true, userId: 2 }),
      provider("local"),
    ]);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows nothing when signed out", () => {
    auth.signedIn = false;
    const { container } = renderButtons("opencode", null, [
      provider("p1", { platform: true }),
    ]);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("PlatformBulkSyncButtons on additive apps", () => {
  it("adds every unadded platform provider through the app add path", async () => {
    vi.mocked(providersApi.getOpenCodeLiveProviderIds).mockResolvedValue([
      "p2",
    ]);
    renderButtons("opencode", null, [
      provider("p1", { platform: true }),
      provider("p2", { platform: true }),
      provider("local"),
    ]);

    await clickWhenEnabled("一键添加");
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith("已添加 1 个平台供应商"),
    );
    expect(providersApi.switch).toHaveBeenCalledWith("p1", "opencode");
    expect(providersApi.switch).not.toHaveBeenCalledWith("p2", "opencode");
    expect(providersApi.removeFromLiveConfig).not.toHaveBeenCalled();
  });

  it("removes added platform providers without deleting their rows", async () => {
    vi.mocked(providersApi.getOpenCodeLiveProviderIds).mockResolvedValue([
      "p2",
    ]);
    renderButtons("opencode", null, [
      provider("p1", { platform: true }),
      provider("p2", { platform: true }),
    ]);

    await clickWhenEnabled("一键移除");
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith("已移除 1 个平台供应商"),
    );
    expect(providersApi.removeFromLiveConfig).toHaveBeenCalledWith(
      "p2",
      "opencode",
    );
    expect(providersApi.switch).not.toHaveBeenCalled();
  });

  it("adds Kimi providers through addToLiveConfig instead of switching", async () => {
    renderButtons("kimi", null, [
      provider("p1", { platform: true, liveConfigManaged: false }),
      provider("p2", { platform: true, liveConfigManaged: true }),
    ]);

    fireEvent.click(screen.getByRole("button", { name: "一键添加" }));
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
    expect(providersApi.addToLiveConfig).toHaveBeenCalledWith("p1", "kimi");
    expect(providersApi.switch).not.toHaveBeenCalled();
  });
});

describe("PlatformBulkSyncButtons in stack view", () => {
  it("adds and removes stack members through the stack API", async () => {
    vi.mocked(proxyApi.getProxyStack).mockResolvedValue(stackWith(["p2"]));
    renderButtons("codex", "stack", [
      provider("p1", { platform: true }),
      provider("p2", { platform: true }),
    ]);

    await clickWhenEnabled("一键添加");
    await waitFor(() =>
      expect(proxyApi.setProxyStackMember).toHaveBeenCalledWith(
        "codex",
        "p1",
        true,
      ),
    );

    await clickWhenEnabled("一键移除");
    await waitFor(() =>
      expect(proxyApi.setProxyStackMember).toHaveBeenCalledWith(
        "codex",
        "p2",
        false,
      ),
    );
  });
});
