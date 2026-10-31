import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "i18next";
import { http, HttpResponse } from "msw";
import { server } from "../../msw/server";
import { emitTauriEvent } from "../../msw/tauriMocks";
import { APP_IDS } from "@/config/appConfig";
import { AppConfigHistoryButton } from "@/zhishu/config-history/AppConfigHistoryButton";
import {
  ConfigChangeHistoryPanel,
  formatChangeTime,
} from "@/zhishu/config-history/ConfigChangeHistoryPanel";
import type {
  ConfigChangeDetail,
  ConfigChangeFileSummary,
  ConfigChangeGroup,
} from "@/zhishu/config-history/api/configHistory";

const m = vi.hoisted(() => ({
  copy: vi.fn(),
  error: vi.fn(),
  success: vi.fn(),
}));
vi.mock("@/lib/clipboard", () => ({ copyText: m.copy }));
vi.mock("@/lib/toast", () => ({
  toast: { error: m.error, success: m.success },
}));

const root = "http://tauri.local";
const changedAt = "2026-10-25T12:34:56+08:00";
const singleFile: ConfigChangeFileSummary = {
  id: "0001",
  path: "/isolated/config.toml",
  beforeExists: true,
  afterExists: true,
  sensitive: false,
};
const group: ConfigChangeGroup = {
  id: "0001",
  app: "codex",
  path: "/isolated/config.toml",
  changedAt,
  action: "switch",
  sensitive: false,
  files: [singleFile],
};
const detail: ConfigChangeDetail = {
  id: "0001",
  app: "codex",
  path: "/isolated/config.toml",
  changedAt,
  action: "switch",
  before: 'model = "old"\n',
  after: 'model = "new"\n',
  sensitive: false,
};
let details: ConfigChangeDetail;
let records: ConfigChangeGroup[];
let restores: string[];
let gets: { id: string; revealSensitive: boolean }[];
let client: QueryClient;
const mount = (node: React.ReactNode) => {
  client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>{node}</QueryClientProvider>,
  );
};
const openRecord = async () => {
  await userEvent.click(
    await screen.findByRole("button", { name: /config.toml/ }),
  );
  await screen.findByRole("tab", { name: "差异对比" });
};

beforeEach(() => {
  records = [group];
  details = { ...detail };
  restores = [];
  gets = [];
  m.copy.mockResolvedValue(undefined);
  server.use(
    http.post(`${root}/list_config_change_history`, async ({ request }) => {
      const { app } = (await request.json()) as { app: string | null };
      return HttpResponse.json(
        records.filter((record) => !app || record.app === app),
      );
    }),
    http.post(`${root}/get_config_change_history`, async ({ request }) => {
      gets.push(
        (await request.json()) as { id: string; revealSensitive: boolean },
      );
      return HttpResponse.json(details);
    }),
    http.post(`${root}/restore_config_change_history`, async ({ request }) => {
      const { ids } = (await request.json()) as { ids: string[] };
      restores.push(...ids);
      return HttpResponse.json({
        files: ids.map((id) => ({
          id,
          path: "/isolated/config.toml",
          changed: true,
          backupPath: "/isolated/backup.json",
        })),
        changed: true,
        pointerRestored: false,
      });
    }),
  );
});

describe("app configuration history", () => {
  it("shows a DB-only provider sync result and exposes its backup without a fictitious file diff", async () => {
    const sync = {
      added: 1,
      updated: 5,
      removed: 0,
      unchanged: 0,
      changedFiles: [],
      backupPath: "/isolated/backups/platform-sync/operation",
      warnings: [],
    };
    const item: ConfigChangeGroup = {
      ...group,
      app: "workbuddy",
      path: sync.backupPath,
      action: "platform_sync",
      sensitive: false,
      sync,
      files: [
        {
          id: "0001",
          path: sync.backupPath,
          beforeExists: false,
          afterExists: false,
          sensitive: false,
        },
      ],
    };
    records = [item];
    details = {
      id: item.id,
      app: item.app,
      path: item.path,
      changedAt: item.changedAt,
      action: item.action,
      before: null,
      after: null,
      sensitive: false,
      sync,
    };
    const opened: string[] = [];
    server.use(
      http.post(`${root}/open_config_sync_backup`, async ({ request }) => {
        opened.push(((await request.json()) as { id: string }).id);
        return new HttpResponse(null, { status: 204 });
      }),
    );
    mount(<ConfigChangeHistoryPanel app="workbuddy" />);
    await userEvent.click(
      await screen.findByRole("button", { name: /供应商同步结果/ }),
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText(
      "未修改 Agent 原生配置；本次仅同步 CCS 内的供应商列表。",
    );
    expect(
      within(dialog).getByText("供应商：新增 1，更新 5，删除 0，未变化 0"),
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole("tab", { name: "差异对比" }),
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole("button", { name: "还原到修改前" }),
    ).not.toBeInTheDocument();
    expect(within(dialog).getByText(sync.backupPath)).toBeInTheDocument();
    await userEvent.click(
      within(dialog).getByRole("button", { name: "复制备份路径" }),
    );
    expect(m.copy).toHaveBeenCalledWith(sync.backupPath);
    await userEvent.click(
      within(dialog).getByRole("button", { name: "打开备份目录" }),
    );
    await waitFor(() => expect(opened).toEqual([item.id]));
    expect(restores).toEqual([]);
  });

  it("lists changed native files and distinguishes a sync without an extra backup", async () => {
    const sync = {
      added: 0,
      updated: 1,
      removed: 0,
      unchanged: 2,
      changedFiles: ["/isolated/.workbuddy/models.json"],
      backupPath: null,
      warnings: ["某分组未同步"],
    };
    const item: ConfigChangeGroup = {
      ...group,
      action: "platform_sync",
      sensitive: false,
      sync,
      files: [
        {
          id: "0001",
          path: "/isolated/config.toml",
          beforeExists: false,
          afterExists: false,
          sensitive: false,
        },
      ],
    };
    records = [item];
    details = {
      id: item.id,
      app: item.app,
      path: item.path,
      changedAt: item.changedAt,
      action: item.action,
      before: null,
      after: null,
      sensitive: false,
      sync,
    };
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await userEvent.click(
      await screen.findByRole("button", { name: /供应商同步结果/ }),
    );
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText(sync.changedFiles[0]);
    expect(
      within(dialog).getByText("本次没有覆盖冲突，未创建额外的同步备份。"),
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole("button", { name: "打开备份目录" }),
    ).not.toBeInTheDocument();
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      "某分组未同步",
    );
  });

  it("shows an aggregation roster summary without a diff or restore", async () => {
    const summary = { added: ["DeepSeek"], removed: [] };
    const item: ConfigChangeGroup = {
      ...group,
      action: "stack",
      summary,
      files: [
        {
          id: "0001",
          path: "/isolated/.codex/cc-switch-model-catalog.json",
          beforeExists: false,
          afterExists: false,
          sensitive: false,
        },
      ],
    };
    records = [item];
    details = {
      id: item.id,
      app: item.app,
      path: item.path,
      changedAt: item.changedAt,
      action: item.action,
      before: null,
      after: null,
      sensitive: false,
      summary,
    };
    mount(<ConfigChangeHistoryPanel app="codex" />);
    const row = await screen.findByRole("button", { name: /添加了 DeepSeek/ });
    expect(row).toHaveTextContent("cc-switch-model-catalog.json");
    expect(within(row).getByText("查看详情")).toBeInTheDocument();
    await userEvent.click(row);
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("添加了 DeepSeek");
    // The path shows in both the dialog description and the affected-file list.
    expect(
      await within(dialog).findAllByText(item.files[0].path),
    ).not.toHaveLength(0);
    expect(
      within(dialog).queryByRole("tab", { name: "差异对比" }),
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole("button", { name: "还原到修改前" }),
    ).not.toBeInTheDocument();
    expect(restores).toEqual([]);
  });

  it("shows one row per operation, reveals full paths on hover, and switches file diffs", async () => {
    // jsdom 没有 scrollIntoView，下拉打开时会调用
    Element.prototype.scrollIntoView = vi.fn();
    const multiFile: ConfigChangeGroup = {
      ...group,
      id: "0002",
      sensitive: true,
      files: [
        {
          id: "0001",
          path: "/isolated/auth.json",
          beforeExists: true,
          afterExists: true,
          sensitive: false,
        },
        {
          id: "0002",
          path: "/isolated/config.toml",
          beforeExists: true,
          afterExists: true,
          sensitive: true,
        },
      ],
    };
    records = [multiFile];
    const byId: Record<string, ConfigChangeDetail> = {
      "0001": {
        id: "0001",
        app: "codex",
        path: "/isolated/auth.json",
        changedAt,
        action: "switch",
        before: "OPENAI_API_KEY=old\n",
        after: "OPENAI_API_KEY=new\n",
        sensitive: false,
      },
      "0002": { ...detail },
    };
    server.use(
      http.post(`${root}/get_config_change_history`, async ({ request }) => {
        const { id, revealSensitive } = (await request.json()) as {
          id: string;
          revealSensitive: boolean;
        };
        gets.push({ id, revealSensitive });
        return HttpResponse.json(byId[id] ?? null);
      }),
    );
    mount(<ConfigChangeHistoryPanel app="codex" />);
    // One operation collapses into a single row that names every file.
    const row = await screen.findByRole("button", { name: /修改了 2 个文件/ });
    expect(row).toHaveTextContent("auth.json");
    expect(row).toHaveTextContent("config.toml");
    // Hovering the truncated line lists the full paths.
    await userEvent.hover(screen.getByText(/修改了 2 个文件/));
    const tooltip = await screen.findByRole("tooltip");
    expect(tooltip).toHaveTextContent("/isolated/auth.json");
    expect(tooltip).toHaveTextContent("/isolated/config.toml");
    // Opening the row starts with the first file; the file can be switched.
    await userEvent.click(row);
    const dialog = await screen.findByRole("dialog");
    const combo = within(dialog).getByRole("combobox");
    expect(combo).toHaveTextContent("auth.json");
    await within(dialog).findByRole("tab", { name: "差异对比" });
    await waitFor(() =>
      expect(gets).toContainEqual({ id: "0001", revealSensitive: false }),
    );
    await userEvent.click(combo);
    await userEvent.click(
      await screen.findByRole("option", { name: "config.toml" }),
    );
    await waitFor(() =>
      expect(gets).toContainEqual({ id: "0002", revealSensitive: false }),
    );
    expect(
      await within(dialog).findByText('model = "new"'),
    ).toBeInTheDocument();
  });

  it("labels platform deletion separately from synchronization", async () => {
    i18n.addResource(
      "zh",
      "translation",
      "platformCatalog.deleteAction",
      "平台供应商删除",
    );
    records = [{ ...group, action: "platform_delete" }];
    mount(<ConfigChangeHistoryPanel app="codex" />);
    expect(await screen.findByText("平台供应商删除")).toBeInTheDocument();
    expect(screen.queryByText("platform_delete")).not.toBeInTheDocument();
  });

  it("labels provider add and remove operations instead of the underlying switch", async () => {
    // The test i18n instance starts empty; register the two action labels the
    // panel is expected to map, so a missing mapping shows the raw action.
    i18n.addResource(
      "zh",
      "translation",
      "settings.configHistory.actions.addProvider",
      "添加供应商",
    );
    i18n.addResource(
      "zh",
      "translation",
      "settings.configHistory.actions.removeProvider",
      "移除供应商",
    );
    records = [
      {
        ...group,
        id: "0002",
        changedAt: "2026-10-25T12:34:57+08:00",
        action: "add_provider",
      },
      {
        ...group,
        id: "0003",
        changedAt: "2026-10-25T12:34:56+08:00",
        action: "remove_provider",
      },
    ];
    mount(<ConfigChangeHistoryPanel app="codex" />);
    expect(await screen.findByText("添加供应商")).toBeInTheDocument();
    expect(screen.getByText("移除供应商")).toBeInTheDocument();
    expect(screen.queryByText("切换供应商")).not.toBeInTheDocument();
    expect(screen.queryByText("add_provider")).not.toBeInTheDocument();
  });

  it.each(APP_IDS)(
    "shows the latest timestamp for %s and opens only its history",
    async (app) => {
      records = [{ ...group, app }];
      mount(<AppConfigHistoryButton app={app} />);
      expect(
        await screen.findByLabelText(
          `最近修改：${formatChangeTime(group.changedAt)}`,
        ),
      ).toHaveAttribute("dateTime", group.changedAt);
      await userEvent.click(
        screen.getByRole("button", { name: /配置修改记录/ }),
      );
      await screen.findByRole("button", { name: /config.toml/ });
      expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: "清空记录" }),
      ).not.toBeInTheDocument();
    },
  );
  it("refreshes the displayed timestamp when a write event arrives", async () => {
    mount(<AppConfigHistoryButton app="codex" />);
    await screen.findByLabelText(
      `最近修改：${formatChangeTime(group.changedAt)}`,
    );
    const time = "2026-10-25T13:14:15+08:00";
    records = [{ ...group, changedAt: time }];
    act(() => emitTauriEvent("config-change-history-updated", "codex"));
    await screen.findByLabelText(`最近修改：${formatChangeTime(time)}`);
  });
  it("clears the timestamp when history is cleared", async () => {
    mount(<AppConfigHistoryButton app="codex" />);
    await screen.findByLabelText(
      `最近修改：${formatChangeTime(group.changedAt)}`,
    );
    records = [];
    act(() => emitTauriEvent("config-change-history-updated", ""));
    await screen.findByText("暂无修改记录");
  });
  it("copies the unified diff and exact before/after contents", async () => {
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await openRecord();
    await userEvent.click(screen.getByRole("button", { name: "复制差异" }));
    expect(m.copy).toHaveBeenLastCalledWith(
      expect.stringContaining('-model = "old"\n+model = "new"'),
    );
    await userEvent.click(screen.getByRole("tab", { name: "完整内容" }));
    await userEvent.click(screen.getByRole("button", { name: "复制修改前" }));
    expect(m.copy).toHaveBeenLastCalledWith(detail.before);
    await userEvent.click(screen.getByRole("button", { name: "复制修改后" }));
    expect(m.copy).toHaveBeenLastCalledWith(detail.after);
  });
  it("copies masked contents until the user reveals sensitive values", async () => {
    records = [{ ...group, sensitive: true }];
    details = {
      ...detail,
      sensitive: true,
      before: "API_KEY=[已隐藏]\nmodel=old",
      after: "API_KEY=[已隐藏]\nmodel=new",
    };
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await openRecord();
    await userEvent.click(screen.getByRole("button", { name: "复制差异" }));
    expect(m.copy.mock.calls.at(-1)?.[0]).not.toContain("secret");
    details = {
      ...details,
      before: "API_KEY=secret-old",
      after: "API_KEY=secret-new",
    };
    await userEvent.click(screen.getByRole("button", { name: "显示敏感值" }));
    await screen.findByRole("button", { name: "隐藏敏感值" });
    await userEvent.click(screen.getByRole("button", { name: "复制差异" }));
    expect(m.copy.mock.calls.at(-1)?.[0]).toContain("secret-new");
    expect(gets).toEqual([
      { id: "0001", revealSensitive: false },
      { id: "0001", revealSensitive: true },
    ]);
  });
  it("requires confirmation and restores by record id without sending masked text", async () => {
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await openRecord();
    await userEvent.click(screen.getByRole("button", { name: "还原到修改前" }));
    expect(restores).toEqual([]);
    const confirm = screen.getByRole("dialog", {
      name: "还原到这次修改之前？",
    });
    await userEvent.click(
      within(confirm).getByRole("button", { name: "还原到修改前" }),
    );
    await waitFor(() => expect(restores).toEqual(["0001"]));
    await waitFor(() =>
      expect(m.success).toHaveBeenCalledWith(
        "已还原 1 个文件，当前配置已备份",
        {
          description: undefined,
        },
      ),
    );
  });
  it("keeps the confirmation open and reports restore conflicts", async () => {
    server.use(
      http.post(`${root}/restore_config_change_history`, () =>
        HttpResponse.text("newer changes", { status: 409 }),
      ),
    );
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await openRecord();
    await userEvent.click(screen.getByRole("button", { name: "还原到修改前" }));
    const confirm = screen.getByRole("dialog", {
      name: "还原到这次修改之前？",
    });
    await userEvent.click(
      within(confirm).getByRole("button", { name: "还原到修改前" }),
    );
    await waitFor(() =>
      expect(m.error).toHaveBeenCalledWith("还原失败", {
        description: "newer changes",
      }),
    );
    expect(confirm).toBeInTheDocument();
  });
  it("shows deletion warning when undoing creation and disables copying missing snapshots", async () => {
    records = [
      {
        ...group,
        files: [{ ...singleFile, beforeExists: false }],
      },
    ];
    details = { ...detail, before: null };
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await openRecord();
    await userEvent.click(screen.getByRole("tab", { name: "完整内容" }));
    expect(screen.getByRole("button", { name: "复制修改前" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "还原到修改前" }));
    expect(
      screen.getByText("修改前文件不存在，还原将删除这些新建文件。"),
    ).toBeInTheDocument();
  });
  it("ignores a detail request that resolves after the dialog closes", async () => {
    let resolve: (value: Response) => void = () => {};
    server.use(
      http.post(
        `${root}/get_config_change_history`,
        () =>
          new Promise<Response>((done) => {
            resolve = done;
          }),
      ),
    );
    mount(<ConfigChangeHistoryPanel app="codex" />);
    await userEvent.click(
      await screen.findByRole("button", { name: /config.toml/ }),
    );
    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "关闭" }));
    await act(async () => {
      resolve(HttpResponse.json(detail));
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText('model = "old"')).not.toBeInTheDocument();
  });
});
