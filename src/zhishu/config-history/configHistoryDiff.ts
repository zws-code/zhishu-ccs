export interface ConfigDiffLine {
  type: "context" | "add" | "del";
  text: string;
  oldNo?: number;
  newNo?: number;
}

const split = (value: string | null): string[] =>
  value ? (value.match(/[^\n]*\n|[^\n]+$/g) ?? []) : [];

/** Bounded line LCS: large replacements fall back to a changed block. */
export function diffConfigText(before: string | null, after: string | null) {
  const old = split(before);
  const next = split(after);
  let start = 0;
  while (
    start < old.length &&
    start < next.length &&
    old[start] === next[start]
  )
    start++;
  let end = 0;
  while (
    end < old.length - start &&
    end < next.length - start &&
    old[old.length - end - 1] === next[next.length - end - 1]
  )
    end++;
  const rows: ConfigDiffLine[] = [];
  let oldNo = 1;
  let newNo = 1;
  const push = (type: ConfigDiffLine["type"], text: string) => {
    rows.push({
      type,
      text,
      oldNo: type !== "add" ? oldNo++ : undefined,
      newNo: type !== "del" ? newNo++ : undefined,
    });
  };
  old.slice(0, start).forEach((line) => push("context", line));
  const a = old.slice(start, old.length - end);
  const b = next.slice(start, next.length - end);
  if ((a.length + 1) * (b.length + 1) <= 1_000_000) {
    const width = b.length + 1;
    const table = new Uint32Array((a.length + 1) * width);
    for (let i = a.length - 1; i >= 0; i--) {
      for (let j = b.length - 1; j >= 0; j--) {
        table[i * width + j] =
          a[i] === b[j]
            ? table[(i + 1) * width + j + 1] + 1
            : Math.max(table[(i + 1) * width + j], table[i * width + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < a.length || j < b.length) {
      if (i < a.length && j < b.length && a[i] === b[j]) {
        push("context", a[i++]);
        j++;
      } else if (
        i < a.length &&
        (j === b.length ||
          table[(i + 1) * width + j] >= table[i * width + j + 1])
      )
        push("del", a[i++]);
      else push("add", b[j++]);
    }
  } else {
    a.forEach((line) => push("del", line));
    b.forEach((line) => push("add", line));
  }
  old.slice(old.length - end).forEach((line) => push("context", line));
  return rows;
}

/** Three lines of context around each change, with real unified hunk ranges. */
export function configDiffHunks(rows: ConfigDiffLine[]) {
  const ranges: { start: number; end: number }[] = [];
  rows.forEach((line, i) => {
    if (line.type === "context") return;
    const start = Math.max(0, i - 3);
    const end = Math.min(rows.length, i + 4);
    const last = ranges.at(-1);
    if (last && start <= last.end) last.end = end;
    else ranges.push({ start, end });
  });
  let oldNo = 1;
  let newNo = 1;
  const positions = rows.map((line) => {
    const position = { oldNo, newNo };
    if (line.type !== "add") oldNo++;
    if (line.type !== "del") newNo++;
    return position;
  });
  return ranges.map(({ start, end }) => {
    const lines = rows.slice(start, end);
    const oldCount = lines.filter((line) => line.type !== "add").length;
    const newCount = lines.filter((line) => line.type !== "del").length;
    const oldStart = positions[start].oldNo - (oldCount === 0 ? 1 : 0);
    const newStart = positions[start].newNo - (newCount === 0 ? 1 : 0);
    return {
      header: `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`,
      lines,
    };
  });
}

export function configUnifiedDiff(
  before: string | null,
  after: string | null,
  path: string,
): string {
  if (before === after) return "";
  const hunks = configDiffHunks(diffConfigText(before, after));
  // Empty-file creation/deletion has no text lines but still changes existence.
  if (!hunks.length)
    return `--- ${before === null ? "/dev/null" : path}\n+++ ${after === null ? "/dev/null" : path}\n`;
  return (
    [
      `--- ${before === null ? "/dev/null" : path}`,
      `+++ ${after === null ? "/dev/null" : path}`,
      ...hunks.flatMap(({ header, lines }) => [
        header,
        ...lines.map(
          (line) =>
            `${line.type === "add" ? "+" : line.type === "del" ? "-" : " "}${line.text.endsWith("\n") ? line.text.slice(0, -1) : line.text + "\n\\ No newline at end of file"}`,
        ),
      ]),
    ].join("\n") + "\n"
  );
}
