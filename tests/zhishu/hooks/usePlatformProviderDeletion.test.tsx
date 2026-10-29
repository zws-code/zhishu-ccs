import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import type { AppId } from "@/lib/api/index";
import type { Provider } from "@/types";
import { usePlatformProviderDeletion } from "@/zhishu/models/usePlatformProviderDeletion";
import {
  platformModelsApi,
  type PlatformDeleteSummary,
} from "@/zhishu/api/platformModels";
import { toast } from "@/lib/toast";
import i18n from "i18next";

const auth = vi.hoisted(() => ({ signedIn: true, userId: 1, syncing: false }));
vi.mock("@/zhishu/auth/PlatformAuthContext", () => ({
  usePlatformAuth: () => ({
    account: auth.signedIn
      ? { baseUrl: "https://platform.test", user: { id: auth.userId } }
      : null,
    catalogSyncing: auth.syncing,
  }),
}));
vi.mock("@/zhishu/api/platformModels", () => ({
  platformModelsApi: { deleteProviders: vi.fn() },
}));
vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));
const provider = (
  id: string,
  origin = "https://platform.test",
  userId = 1,
): Provider => ({
  id,
  name: id,
  settingsConfig: {},
  meta: {
    platformManaged: { origin, userId, entryId: id, model: "test", keyId: 1 },
  },
});
const summary: PlatformDeleteSummary = {
  removed: 2,
  skipped: 0,
  warnings: [],
  nativeFilesChanged: 1,
};
function setup(
  providers: Record<string, Provider> = { one: provider("one") },
  app: AppId = "claude",
) {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const hook = renderHook(
    ({ app }) => usePlatformProviderDeletion(app, providers),
    {
      initialProps: { app },
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    },
  );
  return { ...hook, invalidate };
}
beforeEach(() => {
  vi.clearAllMocks();
  auth.signedIn = true;
  auth.userId = 1;
  auth.syncing = false;
  vi.mocked(platformModelsApi.deleteProviders)
    .mockReset()
    .mockResolvedValue(summary);
  i18n.addResource(
    "zh",
    "translation",
    "platformCatalog.deleteCompleted",
    "已删除 {{removed}} 个平台供应商，保留 {{skipped}} 个",
  );
});

describe("platform provider deletion", () => {
  it("deletes only on request with the current app and account, then refreshes local state", async () => {
    const { result, invalidate } = setup(undefined, "codex");
    expect(platformModelsApi.deleteProviders).not.toHaveBeenCalled();
    act(() => result.current.remove());
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        "已删除 2 个平台供应商，保留 0 个",
      ),
    );
    expect(platformModelsApi.deleteProviders).toHaveBeenCalledWith(
      "codex",
      "https://platform.test",
      1,
    );
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "codex"],
    });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["configChangeHistory"],
    });
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: ["platformUsage"],
    });
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: ["platformCatalog"],
    });
  });
  it("does not enable deletion for manual providers or another account or platform", () => {
    const { result } = setup({
      manual: { id: "manual", name: "manual", settingsConfig: {} },
      other: provider("other", "https://platform.test", 2),
      origin: provider("origin", "https://other.test"),
    });
    expect(result.current.visible).toBe(true);
    expect(result.current.disabled).toBe(true);
    act(() => result.current.remove());
    expect(platformModelsApi.deleteProviders).not.toHaveBeenCalled();
  });
  it("hides unsupported and signed-out actions and disables deletion during automatic sync", () => {
    const { result, rerender } = setup(undefined, "agy");
    expect(result.current.visible).toBe(false);
    rerender({ app: "claude" });
    auth.syncing = true;
    rerender({ app: "claude" });
    expect(result.current.disabled).toBe(true);
    auth.signedIn = false;
    rerender({ app: "claude" });
    expect(result.current.visible).toBe(false);
  });
  it("blocks repeated clicks while deletion is pending", async () => {
    let finish!: (result: PlatformDeleteSummary) => void;
    vi.mocked(platformModelsApi.deleteProviders).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { result } = setup();
    act(() => result.current.remove());
    await waitFor(() => expect(result.current.pending).toBe(true));
    act(() => result.current.remove());
    expect(platformModelsApi.deleteProviders).toHaveBeenCalledTimes(1);
    await act(async () => finish(summary));
  });
  it("shows partial failures with the number retained", async () => {
    vi.mocked(platformModelsApi.deleteProviders).mockResolvedValue({
      ...summary,
      removed: 1,
      skipped: 1,
      warnings: ["当前供应商没有备用项"],
    });
    const { result } = setup();
    act(() => result.current.remove());
    await waitFor(() =>
      expect(toast.warning).toHaveBeenCalledWith(
        "已删除 1 个平台供应商，保留 1 个",
        { description: "当前供应商没有备用项", closeButton: true },
      ),
    );
    expect(toast.success).not.toHaveBeenCalled();
  });
  it("refreshes after failure, hides arbitrary IPC errors and allows retry", async () => {
    vi.mocked(platformModelsApi.deleteProviders).mockRejectedValueOnce(
      new Error("secret-key"),
    );
    const { result, invalidate } = setup();
    act(() => result.current.remove());
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("无法完成平台请求，请稍后重试"),
    );
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "claude"],
    });
    await waitFor(() => expect(result.current.disabled).toBe(false));
    act(() => result.current.remove());
    await waitFor(() => expect(toast.success).toHaveBeenCalled());
  });
  it("ignores results from an account that has logged out", async () => {
    let finish!: (result: PlatformDeleteSummary) => void;
    vi.mocked(platformModelsApi.deleteProviders).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { result, rerender, invalidate } = setup();
    act(() => result.current.remove());
    auth.signedIn = false;
    rerender({ app: "claude" });
    await act(async () => finish(summary));
    expect(invalidate).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });
  it("refreshes the original app if the user navigates while deletion is pending", async () => {
    let finish!: (result: PlatformDeleteSummary) => void;
    vi.mocked(platformModelsApi.deleteProviders).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { result, rerender, invalidate } = setup();
    act(() => result.current.remove());
    rerender({ app: "codex" });
    await act(async () => finish(summary));
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ["providers", "claude"],
    });
    expect(invalidate).not.toHaveBeenCalledWith({
      queryKey: ["providers", "codex"],
    });
  });
});
