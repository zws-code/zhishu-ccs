import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { platformModelsApi } from "@/lib/api/platformModels";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset());

describe("platform synchronization IPC layers", () => {
  it("reads local base data without invoking a synchronization command", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    expect(await platformModelsApi.catalog()).toBeNull();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("platform_get_catalog");
  });

  it("syncs shared platform data independently of any Agent", async () => {
    await platformModelsApi.syncCatalog();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("platform_sync_catalog");
  });

  it("explicitly requests one local group key with its account identity", async () => {
    vi.mocked(invoke).mockResolvedValue("test-only-key");
    expect(
      await platformModelsApi.catalogKey("https://platform.test", 1, 42),
    ).toBe("test-only-key");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("platform_get_catalog_key", {
      baseUrl: "https://platform.test",
      userId: 1,
      groupId: 42,
    });
  });

  it("keeps the Agent sync contract including the all-Agents scope", async () => {
    await platformModelsApi.sync("codex");
    expect(invoke).toHaveBeenLastCalledWith("platform_sync_providers", {
      agent: "codex",
    });
    await platformModelsApi.sync();
    expect(invoke).toHaveBeenLastCalledWith("platform_sync_providers", {
      agent: null,
    });
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
