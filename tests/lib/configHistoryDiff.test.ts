import { describe, expect, it } from "vitest";
import {
  configDiffHunks,
  configUnifiedDiff,
  diffConfigText,
} from "@/lib/configHistoryDiff";

describe("configuration history diff", () => {
  it("aligns unchanged settings and reports only the changed model", () => {
    const rows = diffConfigText(
      'model = "old"\n# keep\nuser = true\n',
      'model = "new"\n# keep\nuser = true\n',
    );
    expect(rows.map((line) => line.type)).toEqual([
      "del",
      "add",
      "context",
      "context",
    ]);
    expect(rows[0].oldNo).toBe(1);
    expect(rows[1].newNo).toBe(1);
  });
  it("keeps creation, deletion and empty files distinct", () => {
    expect(configUnifiedDiff(null, "a\n", "config")).toContain(
      "@@ -0,0 +1,1 @@\n+a",
    );
    expect(configUnifiedDiff("a\n", null, "config")).toContain(
      "@@ -1,1 +0,0 @@\n-a",
    );
    expect(configUnifiedDiff(null, "", "config")).toContain(
      "--- /dev/null\n+++ config",
    );
    expect(configUnifiedDiff("", null, "config")).toContain("+++ /dev/null");
    expect(configUnifiedDiff("", "", "config")).toBe("");
  });
  it("shows final-newline changes and keeps CRLF bytes in copied diffs", () => {
    expect(configUnifiedDiff("key=a", "key=a\n", "config")).toContain(
      "\\ No newline at end of file",
    );
    expect(configUnifiedDiff("a\r\n", "b\r\n", "config")).toContain(
      "-a\r\n+b\r\n",
    );
  });
  it("collapses distant unchanged lines into separate hunks", () => {
    const before = Array.from({ length: 40 }, (_, i) => `line${i}\n`).join("");
    const after = before
      .replace("line1\n", "new1\n")
      .replace("line38\n", "new38\n");
    expect(configDiffHunks(diffConfigText(before, after))).toHaveLength(2);
    expect(configUnifiedDiff(before, after, "config")).not.toContain("line20");
  });
  it("preserves both inputs even for large replacements and repeated lines", () => {
    const pairs = [
      ["a\na\nb", "a\nb\na"],
      ["", "\n"],
      ["\n", null],
      [
        Array.from({ length: 1100 }, (_, i) => `old${i}\n`).join(""),
        Array.from({ length: 1100 }, (_, i) => `new${i}\n`).join(""),
      ],
    ];
    for (const [before, after] of pairs) {
      const rows = diffConfigText(before, after);
      expect(
        rows
          .filter((line) => line.type !== "add")
          .map((line) => line.text)
          .join(""),
      ).toBe(before ?? "");
      expect(
        rows
          .filter((line) => line.type !== "del")
          .map((line) => line.text)
          .join(""),
      ).toBe(after ?? "");
    }
  });
});
