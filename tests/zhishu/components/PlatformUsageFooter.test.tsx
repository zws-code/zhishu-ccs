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
    data: [] as unknown[],
    isFetching: false,
    isError: false,
    error: null as unknown,
    dataUpdatedAt: Date.now(),
    refetch: vi.fn(),
  },
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

describe("PlatformUsageFooter", () => {
  beforeEach(() => {
    mocks.usage.data = [];
    mocks.usage.isFetching = false;
    mocks.usage.isError = false;
    mocks.usage.error = null;
    mocks.usage.refetch.mockClear();
    mocks.auth.refresh.mockClear();
    mocks.openExternal.mockReset().mockResolvedValue(undefined);
    mocks.toastError.mockClear();
  });

  afterEach(() => vi.unstubAllEnvs());

  it("shows daily, weekly, and monthly quota for a subscription group", () => {
    mocks.usage.data = [
      {
        subscriptionId: 7,
        groupId: 14,
        groupName: "lite订阅",
        groupPlatform: "composite",
        expiresAt: "2099-11-01T12:00:00Z",
        expiresInDays: 9,
        unlimited: false,
        daily: {
          limitUsd: 10,
          usedUsd: 2.5,
          remainingUsd: 7.5,
          percentage: 25,
          resetsAt: "2099-10-24T00:00:00Z",
        },
        weekly: {
          limitUsd: 50,
          usedUsd: 25,
          remainingUsd: 25,
          percentage: 50,
          resetsAt: "2099-10-30T00:00:00Z",
        },
        monthly: {
          limitUsd: 100,
          usedUsd: 20,
          remainingUsd: 80,
          percentage: 20,
          resetsAt: "2099-11-01T00:00:00Z",
        },
      },
    ];

    const { container } = renderFooter(14);
    expect(container.textContent).toContain("每日");
    expect(container.textContent).toContain("每周");
    expect(container.textContent).toContain("每月");
  });

  it("renders an unlimited subscription without inventing a percentage", () => {
    mocks.usage.data = [
      {
        subscriptionId: 8,
        groupId: 15,
        groupName: "unlimited",
        groupPlatform: "composite",
        expiresAt: "2099-11-01T12:00:00Z",
        expiresInDays: 9,
        unlimited: true,
      },
    ];

    renderFooter(15);
    expect(screen.getByText("不限量")).toBeInTheDocument();
  });

  it("shows account balance when no provider is selected", () => {
    const { container } = renderFooter(99);
    expect(container.textContent).toContain("余额");
    expect(container.textContent).toContain("12.50 USD");
  });

  it("never shows the current account quota on another account's provider", () => {
    mocks.usage.data = [
      {
        subscriptionId: 7,
        groupId: 14,
        groupName: "lite订阅",
        groupPlatform: "composite",
        expiresAt: "2099-11-01T12:00:00Z",
        expiresInDays: 9,
        unlimited: false,
        daily: {
          limitUsd: 10,
          usedUsd: 2.5,
          remainingUsd: 7.5,
          percentage: 25,
          resetsAt: "2099-10-24T00:00:00Z",
        },
      },
    ];
    const foreign = provider(14);
    foreign.meta!.platformManaged!.userId = 2;
    const { container } = render(
      <I18nextProvider i18n={i18n}>
        <PlatformUsageFooter provider={foreign} />
      </I18nextProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("keeps an expired subscription visible and opens its renewal page", async () => {
    vi.stubEnv("VITE_PLATFORM_WEB_URL", "https://portal.example.com/");
    mocks.usage.data = [
      {
        subscriptionId: 7,
        groupId: 14,
        groupName: "月度订阅",
        groupPlatform: "anthropic",
        expiresAt: "2000-01-01T00:00:00Z",
        expiresInDays: 0,
        unlimited: false,
        status: "expired",
      },
    ];
    const { container } = renderFooter(14, "subscription");
    expect(container.textContent).toContain(zh.quota.planExpired);
    expect(container.textContent).not.toContain("余额");
    fireEvent.click(screen.getByRole("button", { name: "续费订阅" }));
    await waitFor(() =>
      expect(mocks.openExternal).toHaveBeenCalledWith(
        "https://portal.example.com/subscriptions",
      ),
    );
  });

  it("shows suspended subscriptions as unavailable instead of unlimited or balance-billed", () => {
    mocks.usage.data = [
      {
        subscriptionId: 8,
        groupId: 14,
        groupName: "月度订阅",
        groupPlatform: "anthropic",
        expiresAt: "2099-01-01T00:00:00Z",
        expiresInDays: 9,
        unlimited: false,
        status: "suspended",
      },
    ];
    const { container } = renderFooter(14, "subscription");
    expect(container.textContent).toContain("订阅暂不可用");
    expect(container.textContent).not.toContain("不限量");
    expect(container.textContent).not.toContain("余额");
    expect(
      screen.getByRole("button", { name: "管理订阅" }),
    ).toBeInTheDocument();
  });

  it("does not infer balance billing when a known subscription is absent from the response", () => {
    const { container } = renderFooter(14, "subscription");
    expect(container.textContent).toContain("订阅暂不可用");
    expect(container.textContent).not.toContain("余额");
  });

  it("keeps subscription identity when fetching quota fails", () => {
    mocks.usage.isError = true;
    mocks.usage.error = { code: "NETWORK", message: "private-server-detail" };
    const { container } = renderFooter(14, "subscription");
    expect(container.textContent).toContain(zh.quota.failed);
    expect(container.textContent).not.toContain("余额");
    expect(
      screen.getByRole("button", { name: "管理订阅" }),
    ).toBeInTheDocument();
  });

  it("shows an exhausted subscription quota without falling back to account balance", () => {
    mocks.usage.data = [
      {
        subscriptionId: 7,
        groupId: 14,
        groupName: "月度订阅",
        groupPlatform: "anthropic",
        expiresAt: "2099-01-01T00:00:00Z",
        expiresInDays: 9,
        unlimited: false,
        status: "active",
        daily: {
          limitUsd: 10,
          usedUsd: 10,
          remainingUsd: 0,
          percentage: 100,
          resetsAt: "2099-01-01T00:00:00Z",
        },
      },
    ];
    const { container } = renderFooter(14, "subscription");
    expect(container.textContent).toContain("用完");
    expect(container.textContent).not.toContain("余额");
    expect(
      screen.getByRole("button", { name: "管理订阅" }),
    ).toBeInTheDocument();
  });

  it("reports a renewal-page failure without exposing arbitrary errors", async () => {
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
