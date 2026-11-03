import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { PLATFORM_SYNC_APPS } from "@/zhishu/models/platformConfig";

describe("platform synchronization capabilities", () => {
  it("keeps the UI whitelist aligned with the Rust adapter", () => {
    const source = readFileSync(
      resolve("src-tauri/src/zhishu/integrations/ccswitch/platform_models.rs"),
      "utf8",
    );
    const declaration = source.match(
      /const AGENTS: &\[&str\] = &\[([\s\S]*?)\];/,
    );
    expect(declaration).not.toBeNull();
    const backendApps = Array.from(
      declaration![1].matchAll(/"([a-z-]+)"/g),
      ([, app]) => app,
    );
    expect([...PLATFORM_SYNC_APPS].sort()).toEqual(backendApps.sort());
  });
});
