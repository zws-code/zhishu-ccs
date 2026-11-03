import { beforeEach, describe, expect, it } from "vitest";
import {
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
