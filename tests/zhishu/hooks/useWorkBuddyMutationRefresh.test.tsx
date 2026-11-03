import type { ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useAddProviderMutation,
  useUpdateProviderMutation,
  useDeleteProviderMutation,
  useSwitchProviderMutation,
} from "@/lib/query/mutations";

const api = vi.hoisted(() => ({
  add: vi.fn(),
  update: vi.fn(),
  delete: vi.fn(),
  switch: vi.fn(),
  updateTrayMenu: vi.fn(),
}));
vi.mock("@/lib/api", () => ({
  providersApi: api,
  sessionsApi: {},
  settingsApi: {},
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@/lib/toast", () => ({
  toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn() },
}));

function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { wrapper, invalidate };
}

beforeEach(() => {
  for (const mock of Object.values(api))
    mock.mockReset().mockResolvedValue(true);
  api.switch.mockResolvedValue({ warnings: [] });
});

describe("WorkBuddy provider refresh", () => {
  for (const operation of ["add", "update", "delete", "switch"] as const) {
    for (const fail of [false, true]) {
      it(`${operation} refreshes once on ${fail ? "failure" : "success"}`, async () => {
        if (fail)
          api[operation].mockRejectedValueOnce(
            new Error("native write failed"),
          );
        const { wrapper, invalidate } = setup();
        const provider = { id: "fixture", name: "Fixture", settingsConfig: {} };
        const { result } = renderHook(
          () => {
            const add = useAddProviderMutation("workbuddy");
            const update = useUpdateProviderMutation("workbuddy");
            const remove = useDeleteProviderMutation("workbuddy");
            const activate = useSwitchProviderMutation("workbuddy");
            return () => {
              switch (operation) {
                case "add":
                  return add.mutateAsync({
                    ...provider,
                    providerKey: provider.id,
                  });
                case "update":
                  return update.mutateAsync({ provider });
                case "delete":
                  return remove.mutateAsync(provider.id);
                case "switch":
                  return activate.mutateAsync(provider.id);
              }
            };
          },
          { wrapper },
        );
        await act(async () => {
          if (fail)
            await expect(result.current()).rejects.toThrow(
              "native write failed",
            );
          else await result.current();
        });
        expect(api[operation]).toHaveBeenCalledTimes(1);
        const providerRefreshes = invalidate.mock.calls.filter(
          ([filter]) =>
            JSON.stringify(filter?.queryKey) ===
            JSON.stringify(["providers", "workbuddy"]),
        );
        expect(providerRefreshes).toHaveLength(1);
      });
    }
  }
});
