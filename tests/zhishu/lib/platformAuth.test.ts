import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { platformAuthApi } from "@/zhishu/api/platformAuth";

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
  it("passes config baseUrl using the registered IPC contract", async () => {
    await platformAuthApi.config("https://platform.test");
    expect(invoke).toHaveBeenCalledWith("platform_get_client_config", {
      baseUrl: "https://platform.test",
    });
  });
});

it("scopes cleanup logout to the requested platform and user without sending an email", async () => {
  await platformAuthApi.logoutAndClear("https://platform.test", 7);
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenCalledWith("platform_logout_and_clear_account", {
    baseUrl: "https://platform.test",
    userId: 7,
  });
});
