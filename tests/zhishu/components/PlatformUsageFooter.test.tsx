import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createInstance } from "i18next";
import { I18nextProvider, initReactI18next } from "react-i18next";
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { PlatformUsageFooter } from "@/zhishu/models/components/PlatformUsageFooter";
import type { Provider } from "@/types";
import zh from "@/i18n/locales/zh.json";

const i18n = createInstance();

beforeAll(async () => {
  await i18n.use(initReactI18next).init({
    lng: "zh",
    resources: { zh: { translation: zh } },
    interpolation: { escapeValue: false },
  });
});

const mocks = vi.hoisted(() => ({
  openExternal: vi.fn(),
  toastError: vi.fn(),
  auth: {
    account: {
      baseUrl: "https://platform.example.com",
      user: {
        id: 1,
        email: "user@example.com",
        username: "user",
        role: "user",
        balance: 12.5,
      },
    },
    loading: false,
    error: "",
    refresh: vi.fn(),
    acceptLogin: vi.fn(),
    logout: vi.fn(),
  },
  usage: {
    data: undefined as
      | import("@/zhishu/api/platformUsage").PlatformUsageSummary
      | undefined,
    isFetching: false,
    isError: false,
    error: null as unknown,
    dataUpdatedAt: Date.now(),
    refetch: vi.fn(),
  },
}));

vi.mock("@/zhishu/auth/useClientConfig", () => ({
  useClientConfig: () => ({
    data: { subscriptions_url: "https://portal.example.com/subscriptions" },
  }),
}));
vi.mock("@/lib/api/index", () => ({
  settingsApi: { openExternal: mocks.openExternal },
}));
vi.mock("@/lib/toast", () => ({
  toast: { error: mocks.toastError },
}));
vi.mock("@/zhishu/auth/PlatformAuthContext", () => ({
  usePlatformAuth: () => mocks.auth,
}));
vi.mock("@/zhishu/models/query/platformUsage", () => ({
  usePlatformSubscriptionUsage: () => mocks.usage,
}));
vi.mock("@/components/quota/useQuotaDisplay", () => ({
  useQuotaDisplay: () => "left",
}));

function provider(groupId: number): Provider {
  return {
    id: `platform-${groupId}`,
    name: `Platform ${groupId}`,
    category: "aggregator",
    settingsConfig: { env: {} },
    meta: {
      platformManaged: {
        origin: "https://platform.example.com",
        userId: 1,
        entryId: "model",
        model: "model",
        keyId: 1,
        groupId,
      },
    },
  };
}

function renderFooter(groupId: number, subscriptionType?: string) {
  const row = provider(groupId);
  row.meta!.platformManaged!.subscriptionType = subscriptionType;
  return render(
    <I18nextProvider i18n={i18n}>
      <PlatformUsageFooter provider={row} />
    </I18nextProvider>,
  );
}

function window(limit: number | null = null, used = 0) {
  return {
    limit_usd: limit,
    used_usd: used,
    remaining_usd: limit == null ? null : limit - used,
    resets_at: null,
  };
}
function subscription(id = 7, active = true) {
  return {
    id,
    group_id: 14,
    status: active ? "active" : "expired",
    active,
    starts_at: "2026-01-01T00:00:00Z",
    expires_at: active ? "2099-01-01T00:00:00Z" : "2000-01-01T00:00:00Z",
    daily: window(),
    weekly: window(),
    monthly: window(),
  };
}
function group(subscriptionType = "subscription") {
  return {
    id: 14,
    name: "月度订阅",
    platform: "openai",
    status: "active",
    subscription_type: subscriptionType,
    quota: { available: true, reasons: [] as string[] },
    subscriptions: subscriptionType === "subscription" ? [subscription()] : [],
    credential: null,
  };
}
function setGroups(
  groups: import("@/zhishu/api/platformUsage").PlatformUsageGroup[],
) {
  mocks.usage.data = {
    baseUrl: mocks.auth.account.baseUrl,
    userId: 1,
    syncedAt: 1,
    balance: 12.5,
    groups,
  };
}

describe("PlatformUsageFooter dedicated client summary", () => {
  beforeEach(() => {
    setGroups([]);
    mocks.usage.isFetching = false;
    mocks.usage.isError = false;
    mocks.usage.error = null;
    mocks.openExternal.mockReset().mockResolvedValue(undefined);
    mocks.toastError.mockClear();
  });
  afterEach(() => vi.unstubAllEnvs());

  it("shows all limited subscription windows", () => {
    const row = group();
    row.subscriptions[0].daily = window(10, 2.5);
    row.subscriptions[0].weekly = window(50, 25);
    row.subscriptions[0].monthly = window(100, 20);
    setGroups([row]);
    const { container } = renderFooter(14);
    for (const text of ["每日", "每周", "每月"])
      expect(container.textContent).toContain(text);
  });
  it("uses null limits as unlimited only for active, available subscriptions", () => {
    setGroups([group()]);
    renderFooter(14);
    expect(screen.getByText("不限量")).toBeInTheDocument();
  });
  it("uses the read-only summary balance for an available standard group", () => {
    setGroups([group("standard")]);
    const { container } = renderFooter(14);
    expect(container.textContent).toContain("12.50 USD");
  });
  it("does not infer wallet billing from a missing group", () => {
    const { container } = renderFooter(14);
    expect(container.textContent).toContain("暂不可用");
    expect(container.textContent).not.toContain("12.50");
  });
  it.each([
    "key_missing",
    "key_expired",
    "key_disabled",
    "key_quota_exhausted",
    "key_rate_limit_exhausted",
    "balance_insufficient",
    "subscription_expired",
    "subscription_suspended",
    "subscription_quota_exhausted",
  ])("shows %s without unlimited or wallet fallback", (reason) => {
    const row = group();
    row.quota = { available: false, reasons: [reason] };
    setGroups([row]);
    const { container } = renderFooter(14);
    expect(container.textContent).toContain(
      zh.clientQuota[reason as keyof typeof zh.clientQuota],
    );
    expect(container.textContent).not.toContain("不限量");
    expect(container.textContent).not.toContain("12.50");
  });
  it("does not let an expired subscription override another active subscription", () => {
    const row = group();
    row.subscriptions = [subscription(1, false), subscription(2, true)];
    setGroups([row]);
    renderFooter(14);
    expect(screen.getByText("不限量")).toBeInTheDocument();
  });
  it("retains multiple active subscriptions instead of collapsing by group", () => {
    const row = group();
    row.subscriptions.push(subscription(2));
    setGroups([row]);
    renderFooter(14);
    expect(screen.getAllByText("不限量")).toHaveLength(2);
  });
  it("opens the server-provided renewal page", async () => {
    const row = group();
    row.quota = { available: false, reasons: ["subscription_expired"] };
    setGroups([row]);
    renderFooter(14);
    fireEvent.click(screen.getByRole("button", { name: "续费订阅" }));
    await waitFor(() =>
      expect(mocks.openExternal).toHaveBeenCalledWith(
        "https://portal.example.com/subscriptions",
      ),
    );
  });
  it("reports read failures without showing stale unlimited quota", () => {
    setGroups([group()]);
    mocks.usage.isError = true;
    mocks.usage.error = { code: "NETWORK", message: "无法连接平台" };
    const { container } = renderFooter(14);
    expect(container.textContent).toContain(zh.quota.failed);
    expect(container.textContent).not.toContain("不限量");
  });
  it("rejects a late response for a different account", () => {
    setGroups([group()]);
    mocks.usage.data!.userId = 2;
    const { container } = renderFooter(14);
    expect(container.textContent).not.toContain("不限量");
  });
  it("never shows quota for another account's provider", () => {
    const foreign = provider(14);
    foreign.meta!.platformManaged!.userId = 2;
    const { container } = render(
      <I18nextProvider i18n={i18n}>
        <PlatformUsageFooter provider={foreign} />
      </I18nextProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });
  it("reports webpage failures without arbitrary error details", async () => {
    mocks.openExternal.mockRejectedValue(new Error("private-server-detail"));
    renderFooter(14, "subscription");
    fireEvent.click(screen.getByRole("button", { name: "管理订阅" }));
    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        zh.platformUsage.openSubscriptionFailed,
      ),
    );
  });
});
