import type { PropsWithChildren } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { useConfigMembership } from "@/hooks/useConfigMembership";
import { refreshAdditiveLiveState } from "@/lib/query/additiveLiveState";
import { piApi } from "@/lib/api/pi";
import type { Provider } from "@/types";

vi.mock("@/lib/api/pi", () => ({
  piApi: { getCurrentState: vi.fn() },
}));

const provider = (id: string): Provider => ({
  id,
  name: id,
  settingsConfig: {},
});

describe("merged Pi family membership", () => {
  it("reads OMP membership independently and refreshes it after a mutation", async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    client.setQueryData(["pi", "currentState"], {
      enabledProviderIds: ["pi-only"],
      defaultProviderId: null,
    });
    vi.mocked(piApi.getCurrentState).mockResolvedValue({
      enabledProviderIds: ["omp-only"],
      defaultProviderId: null,
    });
    const wrapper = ({ children }: PropsWithChildren) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result, unmount } = renderHook(() => useConfigMembership("omp"), {
      wrapper,
    });
    await waitFor(() => expect(result.current.isPending).toBe(false));
    expect(piApi.getCurrentState).toHaveBeenCalledWith("omp");
    expect(result.current.isInConfig(provider("omp-only"))).toBe(true);
    expect(result.current.isInConfig(provider("pi-only"))).toBe(false);
    vi.mocked(piApi.getCurrentState).mockResolvedValue({
      enabledProviderIds: [],
      defaultProviderId: null,
    });
    await act(() => refreshAdditiveLiveState(client, "omp"));
    await waitFor(() =>
      expect(result.current.isInConfig(provider("omp-only"))).toBe(false),
    );
    expect(client.getQueryState(["pi", "currentState"])?.isInvalidated).toBe(
      false,
    );
    unmount();
    client.clear();
  });

  it("invalidates only the selected app's provider and native-state caches", async () => {
    const client = new QueryClient();
    for (const app of ["pi", "omp"]) {
      client.setQueryData([app, "currentState"], { enabledProviderIds: [] });
      client.setQueryData(["providers", app], {});
    }
    await refreshAdditiveLiveState(client, "omp");
    expect(client.getQueryState(["omp", "currentState"])?.isInvalidated).toBe(
      true,
    );
    expect(client.getQueryState(["providers", "omp"])?.isInvalidated).toBe(
      true,
    );
    expect(client.getQueryState(["pi", "currentState"])?.isInvalidated).toBe(
      false,
    );
    expect(client.getQueryState(["providers", "pi"])?.isInvalidated).toBe(
      false,
    );
    client.clear();
  });
});
