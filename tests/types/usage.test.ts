import { describe, expect, it } from "vitest";
import {
  getCacheWriteAvailability,
  omitsCacheWrites,
  getFreshInputTokens,
} from "@/types/usage";

describe("getCacheWriteAvailability", () => {
  it("distinguishes cache-write support across fixed protocols", () => {
    expect(getCacheWriteAvailability(["claude"])).toBe("ok");
    expect(getCacheWriteAvailability(["codex"])).toBe("ok");
    expect(getCacheWriteAvailability(["claude", "codex"])).toBe("ok");
    expect(getCacheWriteAvailability(["pi"])).toBe("partial");
    expect(getCacheWriteAvailability(["codex", "gemini"])).toBe("partial");
    expect(getCacheWriteAvailability(["gemini", "grokbuild"])).toBe("na");
    expect(getCacheWriteAvailability([])).toBe("ok");
  });

  it("treats a zero cache write as unreported only for Gemini and Grok Build", () => {
    expect(omitsCacheWrites("codex")).toBe(false);
    expect(omitsCacheWrites("claude")).toBe(false);
    expect(omitsCacheWrites("gemini")).toBe(true);
    expect(omitsCacheWrites("grokbuild")).toBe(true);
  });
});

describe("legacy input-token normalization compatibility", () => {
  it("normalizes old inclusive logs while preserving native fresh-input semantics", () => {
    expect(
      getFreshInputTokens({
        appType: "codex",
        inputTokens: 120,
        cacheReadTokens: 100,
      }),
    ).toBe(20);
    expect(
      getFreshInputTokens({
        appType: "claude",
        inputTokens: 20,
        cacheReadTokens: 100,
      }),
    ).toBe(20);
    expect(
      getFreshInputTokens({
        appType: "codex",
        inputTokens: 20,
        cacheReadTokens: 100,
      }),
    ).toBe(20);
  });

  it("uses current backend counts without subtracting cache reads twice, including zero", () => {
    for (const freshInputTokens of [20, 0]) {
      expect(
        getFreshInputTokens({
          appType: "codex",
          inputTokens: 120,
          cacheReadTokens: 100,
          freshInputTokens,
        }),
      ).toBe(freshInputTokens);
    }
  });
});
