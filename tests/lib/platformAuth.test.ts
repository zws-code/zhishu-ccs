import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { platformAuthApi } from "@/lib/api/platformAuth";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => vi.mocked(invoke).mockReset());

describe("platform authentication IPC", () => {
  it("marks visible account reads as activity without changing the account result", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    expect(await platformAuthApi.account()).toBeNull();
    expect(invoke).toHaveBeenCalledWith("platform_get_account", {
      active: true,
    });
  });
  it("marks background reads inactive", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    await platformAuthApi.account(false);
    expect(invoke).toHaveBeenCalledWith("platform_get_account", {
      active: false,
    });
  });
});
