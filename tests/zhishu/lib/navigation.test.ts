import { beforeEach, describe, expect, it } from "vitest";
import {
  normalizeViewForMode,
  parseView,
  readStoredView,
  storeView,
  VIEW_STORAGE_KEY,
} from "@/lib/navigation";

beforeEach(() => localStorage.removeItem(VIEW_STORAGE_KEY));
describe("supplier navigation", () => {
  it("opens suppliers for new users or invalid saved pages", () => {
    expect(readStoredView()).toBe("providers");
    localStorage.setItem(VIEW_STORAGE_KEY, "unknown-page");
    expect(readStoredView()).toBe("providers");
  });
  it("preserves supplier pages and safely ignores an obsolete simple page", () => {
    storeView("providers");
    expect(readStoredView()).toBe("providers");
    localStorage.setItem(VIEW_STORAGE_KEY, "simple");
    expect(readStoredView()).toBe("providers");
    expect(parseView("openclawEnv")).toBe("openclawConfig");
  });
});

describe("simple mode view normalization", () => {
  it("keeps the platform apps page only in simple mode", () => {
    expect(parseView("platformApps")).toBe("platformApps");
    storeView("platformApps");
    expect(readStoredView()).toBe("platformApps");
    expect(normalizeViewForMode("platformApps", true)).toBe("platformApps");
    // 简易模式留下的「应用」开关页，切回高级模式时归一回供应商页
    expect(normalizeViewForMode("platformApps", false)).toBe("providers");
    expect(normalizeViewForMode("providers", false)).toBe("providers");
    expect(normalizeViewForMode("platformCatalog", false)).toBe(
      "platformCatalog",
    );
    expect(normalizeViewForMode("settings", false)).toBe("settings");
  });
});
