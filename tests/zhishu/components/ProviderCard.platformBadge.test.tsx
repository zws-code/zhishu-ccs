import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderCard } from "@/components/providers/ProviderCard";
import type { PlatformCatalog } from "@/zhishu/api/platformModels";
import type { Provider, ProviderMeta } from "@/types";
import { createTestQueryClient } from "../../utils/testQueryClient";

vi.mock("@/components/providers/ProviderActions", () => ({
  ProviderActions: () => null,
}));
vi.mock("@/components/UsageFooter", () => ({
  default: () => null,
}));
vi.mock("@/zhishu/models/components/PlatformUsageFooter", () => ({
  PlatformUsageFooter: () => null,
}));
vi.mock("@/lib/query/failover", () => ({
  useProviderHealth: () => ({ data: undefined }),
}));

const catalogState = vi.hoisted(() => ({
  value: undefined as PlatformCatalog | undefined,
}));
vi.mock("@/zhishu/models/query/platformCatalog", () => ({
  usePlatformCatalog: () => ({ data: catalogState.value }),
}));

function platformCatalog(overrides?: Partial<PlatformCatalog>): PlatformCatalog {
  return {
    baseUrl: "https://platform.example.com",
    userId: 1,
    syncedAt: 0,
    groups: [
      {
        id: 7,
        name: "标准",
        platform: "zhishu",
        subscriptionType: "standard",
        failed: false,
        error: null,
        models: [
          { id: "m1", name: "Model 1" },
          { id: "m2", name: "Model 2" },
          { id: "m3", name: "Model 3" },
        ],
      },
    ],
    warnings: [],
    ...overrides,
  };
}

function renderCard(platformManaged: ProviderMeta["platformManaged"]) {
  const provider: Provider = {
    id: "platform-provider",
    name: "智枢平台 · 标准",
    category: "custom",
    settingsConfig: {
      env: { ANTHROPIC_BASE_URL: "https://example.com" },
    },
    meta: platformManaged ? { platformManaged } : {},
  };
  return render(
    <QueryClientProvider client={createTestQueryClient()}>
      <ProviderCard
        provider={provider}
        appId="claude"
        isCurrent={false}
        presentation={{ chips: [], buttons: [] }}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onConfigureUsage={vi.fn()}
        onOpenWebsite={vi.fn()}
      />
    </QueryClientProvider>,
  );
}

describe("ProviderCard platform badge", () => {
  it("marks platform-managed providers with a platform chip", () => {
    renderCard({
      origin: "https://platform.example.com",
      userId: 1,
      entryId: "entry-1",
      model: "standard",
      keyId: 1,
    });

    expect(screen.getByText("平台")).toBeInTheDocument();
    expect(
      screen.getByTitle("来自平台基础数据同步的供应商，API Key 由平台托管"),
    ).toBeInTheDocument();
  });

  it("does not mark local providers with a platform chip", () => {
    renderCard(undefined);

    expect(screen.queryByText("平台")).not.toBeInTheDocument();
  });
});

describe("ProviderCard platform model count", () => {
  const managed = {
    origin: "https://platform.example.com",
    userId: 1,
    entryId: "entry-1",
    model: "standard",
    keyId: 1,
    groupId: 7,
  };

  it("shows the catalog model count for the matching group", () => {
    catalogState.value = platformCatalog();
    renderCard(managed);

    expect(screen.getByText("3 个模型")).toBeInTheDocument();
  });

  it("does not show a count for another account's catalog", () => {
    catalogState.value = platformCatalog({ userId: 2 });
    renderCard(managed);

    expect(screen.queryByText("3 个模型")).not.toBeInTheDocument();
  });

  it("does not show a count when the group failed to sync", () => {
    catalogState.value = platformCatalog({
      groups: [platformCatalog().groups[0]!, { ...platformCatalog().groups[0]!, id: 8, failed: true, models: [] }],
    });
    renderCard({ ...managed, groupId: 8 });

    expect(screen.queryByText("0 个模型")).not.toBeInTheDocument();
  });
});
