import { fireEvent, screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WorkBuddyProviderForm } from "@/components/providers/forms/WorkBuddyProviderForm";
import { server } from "../msw/server";
import { renderWithQueryClient as render } from "../utils/testQueryClient";

describe("WorkBuddyProviderForm", () => {
  beforeEach(() => {
    server.use(
      http.post("http://tauri.local/get_providers", () =>
        HttpResponse.json({}),
      ),
    );
  });

  it("submits one provider with an API key and a separate model list", async () => {
    const onSubmit = vi.fn();
    render(
      <WorkBuddyProviderForm
        appId="workbuddy"
        submitLabel="save-workbuddy"
        onSubmit={onSubmit}
        onCancel={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByLabelText("provider.name"), {
      target: { value: "DeepSeek" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "API endpoint" }), {
      target: { value: "https://api.deepseek.com/v1" },
    });
    fireEvent.change(screen.getByLabelText("API Key"), {
      target: { value: "sk-test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add model" }));
    fireEvent.change(screen.getByLabelText("Model ID"), {
      target: { value: "deepseek-v4-pro" },
    });
    fireEvent.change(screen.getByLabelText("Display name"), {
      target: { value: "DeepSeek V4 Pro" },
    });

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "save-workbuddy" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "save-workbuddy" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    const values = onSubmit.mock.calls[0][0];
    expect(values.providerKey).toMatch(/^workbuddy-/);
    expect(values.providerKey).not.toBe("deepseek-v4-pro");
    expect(values.name).toBe("DeepSeek");
    expect(JSON.parse(values.settingsConfig)).toMatchObject({
      url: "https://api.deepseek.com/v1",
      apiKey: "sk-test",
      vendor: "",
      models: [
        {
          id: "deepseek-v4-pro",
          name: "DeepSeek V4 Pro",
          supportsToolCall: true,
          supportsImages: false,
          supportsReasoning: false,
          onlyReasoning: false,
          useCustomProtocol: false,
        },
      ],
    });
    expect(JSON.parse(values.settingsConfig).id).toBeUndefined();
  });

  it("allows a loopback provider without an API key", async () => {
    const onSubmit = vi.fn();
    render(
      <WorkBuddyProviderForm
        appId="workbuddy"
        submitLabel="save-local"
        onSubmit={onSubmit}
        onCancel={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByLabelText("provider.name"), {
      target: { value: "Local provider" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "API endpoint" }), {
      target: { value: "http://127.0.0.1:11434/v1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add model" }));
    fireEvent.change(screen.getByLabelText("Model ID"), {
      target: { value: "local-model" },
    });
    fireEvent.change(screen.getByLabelText("Display name"), {
      target: { value: "Local model" },
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "save-local" })).toBeEnabled(),
    );
  });

  it("preserves unknown provider and model fields while editing", async () => {
    const onSubmit = vi.fn();
    render(
      <WorkBuddyProviderForm
        appId="workbuddy"
        providerId="workbuddy-existing"
        initialData={{
          name: "Existing provider",
          settingsConfig: {
            url: "https://example.com/v1",
            apiKey: "sk-existing",
            vendor: "custom",
            futureProviderField: { keep: true },
            models: [
              {
                id: "existing-model",
                name: "Existing model",
                futureModelField: { keep: true },
              },
            ],
          },
          category: "custom",
        }}
        submitLabel="save-existing"
        onSubmit={onSubmit}
        onCancel={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "save-existing" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "save-existing" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    const config = JSON.parse(onSubmit.mock.calls[0][0].settingsConfig);
    expect(config.futureProviderField).toEqual({ keep: true });
    expect(config.models[0].futureModelField).toEqual({ keep: true });
    expect(onSubmit.mock.calls[0][0].providerKey).toBe("workbuddy-existing");
  });
});
