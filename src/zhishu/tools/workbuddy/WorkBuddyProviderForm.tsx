import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { ChevronRight, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Form } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/lib/toast";
import {
  fetchModelsForConfig,
  showFetchModelsError,
  type FetchedModel,
} from "@/lib/api/model-fetch";
import { useProvidersQuery } from "@/lib/query/queries";
import type { ProviderFormData } from "@/lib/schemas/provider";
import { BasicFormFields } from "@/components/providers/forms/BasicFormFields";
import { FetchedModelPicker } from "@/components/providers/forms/FetchedModelPicker";
import {
  ApiKeySection,
  EndpointField,
} from "@/components/providers/forms/shared/index";
import type { ProviderFormProps } from "@/components/providers/forms/ProviderForm";

type WorkBuddyModelConfig = Record<string, unknown> & {
  id?: string;
  name?: string;
  maxInputTokens?: number | string;
  maxOutputTokens?: number | string;
  temperature?: number | string;
  supportsToolCall?: boolean;
  supportsImages?: boolean;
  supportsReasoning?: boolean;
  onlyReasoning?: boolean;
  useCustomProtocol?: boolean;
  reasoning?: Record<string, unknown>;
};

type WorkBuddyModelRow = {
  key: string;
  config: WorkBuddyModelConfig;
};

const asObject = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const stringValue = (value: unknown): string =>
  typeof value === "string" ? value : "";

const numberValue = (value: unknown): string =>
  typeof value === "number" && Number.isFinite(value)
    ? String(value)
    : typeof value === "string"
      ? value
      : "";

const parseOptionalNumber = (value: string): number | undefined => {
  if (!value.trim()) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
};

const isHttpUrl = (value: string): boolean => {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
};

const isLoopbackEndpoint = (value: string): boolean => {
  try {
    const host = new URL(value).hostname.toLowerCase();
    return (
      host === "localhost" ||
      host.endsWith(".localhost") ||
      host === "127.0.0.1" ||
      host === "::1" ||
      host === "[::1]"
    );
  } catch {
    return false;
  }
};

const newModelRow = (model: WorkBuddyModelConfig): WorkBuddyModelRow => ({
  key: crypto.randomUUID(),
  config: {
    supportsToolCall: true,
    supportsImages: false,
    supportsReasoning: false,
    onlyReasoning: false,
    useCustomProtocol: false,
    ...model,
  },
});

const initialModels = (
  providerConfig: Record<string, unknown>,
): WorkBuddyModelRow[] => {
  const configured = Array.isArray(providerConfig.models)
    ? providerConfig.models.filter(
        (model): model is Record<string, unknown> =>
          Boolean(model) && typeof model === "object" && !Array.isArray(model),
      )
    : [];
  if (configured.length > 0) {
    return configured.map((model) => newModelRow(model));
  }
  // Backward compatibility for the first WorkBuddy implementation where each
  // provider row was itself a single model.
  if (typeof providerConfig.id === "string" && providerConfig.id.trim()) {
    return [newModelRow(providerConfig)];
  }
  return [];
};

const cleanModel = (model: WorkBuddyModelConfig): WorkBuddyModelConfig => {
  const maxInput = parseOptionalNumber(numberValue(model.maxInputTokens));
  const maxOutput = parseOptionalNumber(numberValue(model.maxOutputTokens));
  const temperature = parseOptionalNumber(numberValue(model.temperature));
  const supportsReasoning = model.supportsReasoning === true;
  const next: WorkBuddyModelConfig = {
    ...model,
    id: stringValue(model.id).trim(),
    name: stringValue(model.name).trim(),
    supportsToolCall: model.supportsToolCall ?? true,
    supportsImages: model.supportsImages ?? false,
    supportsReasoning,
    onlyReasoning: supportsReasoning && model.onlyReasoning === true,
    useCustomProtocol: model.useCustomProtocol ?? false,
  };
  if (maxInput !== undefined) next.maxInputTokens = maxInput;
  else delete next.maxInputTokens;
  if (maxOutput !== undefined) next.maxOutputTokens = maxOutput;
  else delete next.maxOutputTokens;
  if (temperature !== undefined) next.temperature = temperature;
  else delete next.temperature;
  if (!supportsReasoning) delete next.reasoning;
  return next;
};

export function WorkBuddyProviderForm({
  providerId,
  initialData,
  onSubmit,
  onCancel,
  submitLabel,
  showButtons = true,
  onSubmittingChange,
  onSubmitReadyChange,
}: ProviderFormProps) {
  const { t } = useTranslation();
  const initialConfig = useMemo(
    () => asObject(initialData?.settingsConfig),
    [initialData?.settingsConfig],
  );
  const isEdit = Boolean(initialData);
  const [providerKey] = useState(
    () => providerId || `workbuddy-${crypto.randomUUID().slice(0, 8)}`,
  );
  const [endpoint, setEndpoint] = useState(stringValue(initialConfig.url));
  const [apiKey, setApiKey] = useState(stringValue(initialConfig.apiKey));
  const [vendor, setVendor] = useState(stringValue(initialConfig.vendor));
  const [models, setModels] = useState<WorkBuddyModelRow[]>(() =>
    initialModels(initialConfig),
  );
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [fetchedModels, setFetchedModels] = useState<FetchedModel[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const form = useForm<ProviderFormData>({
    defaultValues: {
      name: initialData?.name ?? "",
      notes: initialData?.notes ?? "",
      websiteUrl: initialData?.websiteUrl ?? "",
      icon: initialData?.icon ?? "",
      iconColor: initialData?.iconColor ?? "",
      settingsConfig: JSON.stringify(initialConfig, null, 2),
    },
  });
  const name = form.watch("name");
  const { data: existing } = useProvidersQuery("workbuddy");
  const idTaken =
    !isEdit &&
    Object.prototype.hasOwnProperty.call(
      existing?.providers ?? {},
      providerKey,
    );
  const endpointValid = isHttpUrl(endpoint.trim());
  const apiKeyRequired = endpointValid && !isLoopbackEndpoint(endpoint.trim());

  const modelIds = models.map((model) => stringValue(model.config.id).trim());
  const seenModelIds = new Set<string>();
  const duplicateModelIds = new Set<string>();
  for (const id of modelIds) {
    if (!id) continue;
    if (seenModelIds.has(id)) duplicateModelIds.add(id);
    seenModelIds.add(id);
  }
  const modelsValid =
    models.length > 0 &&
    models.every((model, index) => {
      const config = model.config;
      const input = parseOptionalNumber(numberValue(config.maxInputTokens));
      const output = parseOptionalNumber(numberValue(config.maxOutputTokens));
      const temperature = parseOptionalNumber(numberValue(config.temperature));
      const blankInput = !numberValue(config.maxInputTokens).trim();
      const blankOutput = !numberValue(config.maxOutputTokens).trim();
      const blankTemperature = !numberValue(config.temperature).trim();
      return (
        modelIds[index] !== "" &&
        stringValue(config.name).trim() !== "" &&
        !duplicateModelIds.has(modelIds[index]) &&
        (blankInput || (input !== undefined && input > 0)) &&
        (blankOutput || (output !== undefined && output > 0)) &&
        (blankTemperature || temperature !== undefined)
      );
    });
  const ready = Boolean(
    name.trim() &&
      !idTaken &&
      endpointValid &&
      (!apiKeyRequired || apiKey.trim()) &&
      modelsValid,
  );

  useEffect(() => {
    onSubmitReadyChange?.(ready);
  }, [onSubmitReadyChange, ready]);

  const updateModel = (key: string, patch: Partial<WorkBuddyModelConfig>) => {
    setModels((current) =>
      current.map((model) =>
        model.key === key
          ? { ...model, config: { ...model.config, ...patch } }
          : model,
      ),
    );
  };

  const addModel = () => {
    setModels((current) => [...current, newModelRow({ id: "", name: "" })]);
  };

  const removeModel = (key: string) => {
    setModels((current) => current.filter((model) => model.key !== key));
    setExpanded((current) => {
      const next = new Set(current);
      next.delete(key);
      return next;
    });
  };

  const fetchModels = () => {
    if (!endpoint.trim() || (!apiKey.trim() && apiKeyRequired)) {
      showFetchModelsError(null, t, {
        hasBaseUrl: Boolean(endpoint.trim()),
        hasApiKey: Boolean(apiKey.trim()),
      });
      return;
    }
    setFetchingModels(true);
    fetchModelsForConfig(endpoint.trim(), apiKey.trim())
      .then((fetched) => {
        setFetchedModels(fetched);
        if (fetched.length === 0) {
          toast.info(t("providerForm.fetchModelsEmpty"));
        } else {
          toast.success(
            t("providerForm.fetchModelsSuccess", { count: fetched.length }),
          );
        }
      })
      .catch((fetchError) => {
        console.warn("[WorkBuddy] Failed to fetch models:", fetchError);
        showFetchModelsError(fetchError, t);
      })
      .finally(() => setFetchingModels(false));
  };

  const applyFetchedModels = ({
    add,
    remove,
  }: {
    add: string[];
    remove: string[];
  }) => {
    const removed = new Set(remove);
    setModels((current) => {
      const kept = current.filter(
        (model) => !removed.has(stringValue(model.config.id)),
      );
      const existingIds = new Set(
        kept.map((model) => stringValue(model.config.id)),
      );
      return [
        ...kept,
        ...add
          .filter((id) => !existingIds.has(id))
          .map((id) => newModelRow({ id, name: id })),
      ];
    });
  };

  const submit = async (identity: ProviderFormData) => {
    if (!ready || busy) return;
    setBusy(true);
    onSubmittingChange?.(true);
    setError("");
    try {
      const providerConfig: Record<string, unknown> = {
        ...initialConfig,
        url: endpoint.trim(),
        apiKey: apiKey.trim(),
        vendor: vendor.trim(),
        models: models.map((model) => cleanModel(model.config)),
      };
      delete providerConfig.id;
      delete providerConfig.name;
      delete providerConfig.supportsToolCall;
      delete providerConfig.supportsImages;
      delete providerConfig.supportsReasoning;
      delete providerConfig.onlyReasoning;
      delete providerConfig.useCustomProtocol;
      delete providerConfig.maxInputTokens;
      delete providerConfig.maxOutputTokens;
      delete providerConfig.temperature;
      delete providerConfig.reasoning;

      await onSubmit({
        ...identity,
        name: identity.name.trim(),
        settingsConfig: JSON.stringify(providerConfig),
        providerKey,
        presetCategory: initialData?.category ?? "custom",
        meta: initialData?.meta,
      });
    } catch (submitError) {
      setError(
        submitError instanceof Error
          ? submitError.message
          : String(submitError),
      );
    } finally {
      setBusy(false);
      onSubmittingChange?.(false);
    }
  };

  return (
    <Form {...form}>
      <form
        id="provider-form"
        className="space-y-6"
        onSubmit={form.handleSubmit(submit)}
      >
        {error ? (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        ) : null}
        <fieldset disabled={busy} className="min-w-0 space-y-6 border-0 p-0">
          <BasicFormFields form={form} />

          <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_12rem]">
            <EndpointField
              id="workbuddy-endpoint"
              label={t("workbuddy.endpoint", {
                defaultValue: "API endpoint",
              })}
              value={endpoint}
              onChange={setEndpoint}
              placeholder="https://api.example.com/v1"
              help={t("workbuddy.endpointHint", {
                defaultValue:
                  "WorkBuddy appends /chat/completions unless custom protocol passthrough is enabled.",
              })}
              showManageButton={false}
            />
            <div className="space-y-2">
              <Label htmlFor="workbuddy-vendor">
                {t("workbuddy.vendor", { defaultValue: "Vendor" })}
              </Label>
              <Input
                id="workbuddy-vendor"
                value={vendor}
                onChange={(event) => setVendor(event.target.value)}
                placeholder="custom"
                autoComplete="off"
              />
            </div>
          </div>

          <ApiKeySection
            id="workbuddy-api-key"
            value={apiKey}
            onChange={setApiKey}
            category={initialData?.category ?? "custom"}
            shouldShowLink={false}
            websiteUrl=""
            required={apiKeyRequired}
            placeholder={{
              official: t("workbuddy.apiKeyOfficial", {
                defaultValue: "Not required for a loopback endpoint",
              }),
              thirdParty: t("workbuddy.apiKeyPlaceholder", {
                defaultValue: "Enter the upstream API key",
              }),
            }}
          />

          <section className="space-y-3 border-l border-border pl-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <Label>
                  {t("workbuddy.models", { defaultValue: "Models" })}
                </Label>
                <p className="text-xs text-fg-2">
                  {t("workbuddy.modelsHint", {
                    defaultValue:
                      "Models are separate from the provider identity. Fetch them from the endpoint or add them manually.",
                  })}
                </p>
              </div>
              <div className="flex gap-1">
                <FetchedModelPicker
                  models={fetchedModels}
                  configuredModelIds={modelIds.filter(Boolean)}
                  onApply={applyFetchedModels}
                  onFetch={fetchModels}
                  isFetching={fetchingModels}
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={addModel}
                  className="h-7 gap-1"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {t("workbuddy.addModel", { defaultValue: "Add model" })}
                </Button>
              </div>
            </div>

            {!endpointValid && endpoint.trim() ? (
              <p className="text-xs text-destructive">
                {t("workbuddy.invalidEndpoint", {
                  defaultValue: "Enter an absolute HTTP(S) endpoint",
                })}
              </p>
            ) : null}

            {models.length === 0 ? (
              <p className="py-2 text-sm text-fg-2">
                {t("workbuddy.noModels", {
                  defaultValue: "No models configured. Add or fetch a model.",
                })}
              </p>
            ) : (
              <div className="space-y-3">
                <div className="flex items-center gap-2 px-1 text-xs text-fg-2">
                  <span className="w-9" />
                  <span className="min-w-0 flex-1">Model ID</span>
                  <span className="min-w-0 flex-1">
                    {t("workbuddy.modelName", {
                      defaultValue: "Display name",
                    })}
                  </span>
                  <span className="w-9" />
                </div>
                {models.map((model) => {
                  const modelId = stringValue(model.config.id);
                  const isExpanded = expanded.has(model.key);
                  const detailsId = `workbuddy-model-details-${model.key}`;
                  const duplicate =
                    modelId !== "" && duplicateModelIds.has(modelId.trim());
                  return (
                    <div key={model.key} className="space-y-2">
                      <div className="flex items-center gap-2">
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          onClick={() =>
                            setExpanded((current) => {
                              const next = new Set(current);
                              if (next.has(model.key)) next.delete(model.key);
                              else next.add(model.key);
                              return next;
                            })
                          }
                          aria-label={t("workbuddy.toggleModelDetails", {
                            defaultValue: "Toggle model details",
                          })}
                          aria-expanded={isExpanded}
                          aria-controls={detailsId}
                          className="h-9 w-9 shrink-0"
                        >
                          <ChevronRight
                            className={`h-4 w-4 transition-transform motion-reduce:transition-none ${
                              isExpanded ? "rotate-90" : ""
                            }`}
                          />
                        </Button>
                        <Input
                          value={modelId}
                          onChange={(event) =>
                            updateModel(model.key, { id: event.target.value })
                          }
                          placeholder="model-id"
                          aria-label="Model ID"
                          className={
                            duplicate
                              ? "min-w-0 flex-1 border-destructive"
                              : "min-w-0 flex-1"
                          }
                          required
                        />
                        <Input
                          value={stringValue(model.config.name)}
                          onChange={(event) =>
                            updateModel(model.key, { name: event.target.value })
                          }
                          placeholder={t("workbuddy.modelNamePlaceholder", {
                            defaultValue: "Model display name",
                          })}
                          aria-label={t("workbuddy.modelName", {
                            defaultValue: "Display name",
                          })}
                          className="min-w-0 flex-1"
                          required
                        />
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          onClick={() => removeModel(model.key)}
                          aria-label={t("workbuddy.removeModel", {
                            defaultValue: "Remove model",
                          })}
                          className="h-9 w-9 shrink-0 text-fg-2 hover:text-destructive"
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                      {duplicate ? (
                        <p className="ml-11 text-xs text-destructive">
                          {t("workbuddy.duplicateModel", {
                            defaultValue:
                              "Model IDs must be unique within this provider",
                          })}
                        </p>
                      ) : null}
                      {isExpanded ? (
                        <div
                          id={detailsId}
                          className="ml-11 grid gap-3 border-l border-border pl-4 sm:grid-cols-2"
                        >
                          {[
                            [
                              "supportsToolCall",
                              t("workbuddy.supportsToolCall", {
                                defaultValue: "Tool calls",
                              }),
                            ],
                            [
                              "supportsImages",
                              t("workbuddy.supportsImages", {
                                defaultValue: "Image input",
                              }),
                            ],
                            [
                              "supportsReasoning",
                              t("workbuddy.supportsReasoning", {
                                defaultValue: "Reasoning",
                              }),
                            ],
                            [
                              "onlyReasoning",
                              t("workbuddy.onlyReasoning", {
                                defaultValue: "Reasoning only",
                              }),
                            ],
                            [
                              "useCustomProtocol",
                              t("workbuddy.useCustomProtocol", {
                                defaultValue: "Custom protocol passthrough",
                              }),
                            ],
                          ].map(([field, label]) => (
                            <label
                              key={field}
                              className="flex items-center justify-between gap-3 rounded-md bg-subtle px-3 py-2"
                            >
                              <span className="text-sm">{label}</span>
                              <Switch
                                checked={
                                  model.config[
                                    field as keyof WorkBuddyModelConfig
                                  ] === true
                                }
                                disabled={
                                  field === "onlyReasoning" &&
                                  model.config.supportsReasoning !== true
                                }
                                onCheckedChange={(checked) =>
                                  updateModel(model.key, {
                                    [field]: checked,
                                    ...(field === "supportsReasoning" &&
                                    !checked
                                      ? { onlyReasoning: false }
                                      : {}),
                                  })
                                }
                              />
                            </label>
                          ))}
                          <div className="space-y-1">
                            <Label className="text-xs text-fg-2">
                              {t("workbuddy.maxInputTokens", {
                                defaultValue: "Max input tokens",
                              })}
                            </Label>
                            <Input
                              type="number"
                              min="1"
                              value={numberValue(model.config.maxInputTokens)}
                              onChange={(event) =>
                                updateModel(model.key, {
                                  maxInputTokens: event.target.value,
                                })
                              }
                            />
                          </div>
                          <div className="space-y-1">
                            <Label className="text-xs text-fg-2">
                              {t("workbuddy.maxOutputTokens", {
                                defaultValue: "Max output tokens",
                              })}
                            </Label>
                            <Input
                              type="number"
                              min="1"
                              value={numberValue(model.config.maxOutputTokens)}
                              onChange={(event) =>
                                updateModel(model.key, {
                                  maxOutputTokens: event.target.value,
                                })
                              }
                            />
                          </div>
                          <div className="space-y-1">
                            <Label className="text-xs text-fg-2">
                              {t("workbuddy.temperature", {
                                defaultValue: "Temperature",
                              })}
                            </Label>
                            <Input
                              type="number"
                              step="0.1"
                              value={numberValue(model.config.temperature)}
                              onChange={(event) =>
                                updateModel(model.key, {
                                  temperature: event.target.value,
                                })
                              }
                            />
                          </div>
                        </div>
                      ) : null}
                    </div>
                  );
                })}
              </div>
            )}
          </section>
        </fieldset>
        {showButtons ? (
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={onCancel}
              disabled={busy}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={busy || !ready}>
              {submitLabel}
            </Button>
          </div>
        ) : null}
      </form>
    </Form>
  );
}
