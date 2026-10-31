import { useState } from "react";
import { ChevronDown, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import type { PlatformCatalogModel } from "@/zhishu/api/platformModels";

/** 本机模型目录的展示与筛选，不发起平台或 Key 请求。 */
export function PlatformCatalogModels({
  models,
}: {
  models: PlatformCatalogModel[];
}) {
  const { t } = useTranslation();
  const [search, setSearch] = useState("");
  const query = search.trim().toLocaleLowerCase();
  const filtered = models.filter(
    (model) =>
      !query ||
      `${model.name}\n${model.id}`.toLocaleLowerCase().includes(query),
  );
  return (
    <>
      <div className="shrink-0 space-y-2 px-6 pb-3 pt-4">
        <div className="relative">
          <Search
            aria-hidden
            className="pointer-events-none absolute start-3 top-2.5 h-4 w-4 text-fg-3"
          />
          <Input
            type="search"
            aria-label={t("platformDetails.search")}
            placeholder={t("platformDetails.search")}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            className="h-9 ps-9"
          />
        </div>
        <p role="status" className="text-caption text-fg-3">
          {t("platformDetails.modelCount", {
            shown: filtered.length,
            total: models.length,
          })}
        </p>
      </div>
      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto overscroll-contain px-6 pb-6">
        {filtered.map((model) => (
          <ModelDetails key={model.id} model={model} />
        ))}
        {!filtered.length && (
          <div className="rounded-lg border border-dashed border-border px-4 py-12 text-center text-body text-fg-3">
            {models.length
              ? t("platformDetails.noMatches")
              : t("platformCatalog.noModels", "暂无可展示的模型数据")}
          </div>
        )}
      </div>
    </>
  );
}

function ModelDetails({ model }: { model: PlatformCatalogModel }) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const unknown = t("platformCatalog.notReported", "未提供");
  const yesNo = (value?: boolean | null) =>
    value == null
      ? unknown
      : value
        ? t("platformCatalog.supported", "支持")
        : t("platformCatalog.unsupported", "不支持");
  const number = (value?: number | null) =>
    value != null && value > 0
      ? value.toLocaleString(i18n.resolvedLanguage ?? i18n.language)
      : unknown;
  const list = (values?: string[]) =>
    values?.length ? values.join(" · ") : unknown;
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="overflow-hidden rounded-lg border border-border bg-surface"
    >
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center justify-between gap-3 px-4 py-3 text-start transition-colors hover:bg-subtle focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
          aria-label={t("platformCatalog.toggleModel", {
            defaultValue: "{{name}} 的模型参数",
            name: model.name,
          })}
        >
          <span className="min-w-0 space-y-1">
            <span className="block break-all text-body font-medium">
              {model.name}
            </span>
            {model.name !== model.id && (
              <code className="block break-all text-caption text-fg-3">
                {model.id}
              </code>
            )}
          </span>
          <ChevronDown
            className={`h-4 w-4 shrink-0 text-fg-3 transition-transform ${open ? "rotate-180" : ""}`}
          />
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="space-y-3 border-t border-border px-4 py-4">
          {model.description && (
            <p className="whitespace-pre-wrap break-words text-caption leading-relaxed text-fg-2">
              {model.description}
            </p>
          )}
          <dl className="grid grid-cols-2 gap-2 text-caption">
            {[
              [
                t("platformCatalog.contextWindow", "上下文窗口"),
                number(model.contextWindow),
              ],
              [
                t("platformCatalog.maxOutput", "最大输出 Token"),
                number(model.maxOutputTokens),
              ],
              [
                t("platformCatalog.endpoints", "接口协议"),
                list(model.endpoints),
              ],
              [
                t("platformCatalog.inputModalities", "输入模态"),
                list(model.inputModalities),
              ],
              [
                t("platformCatalog.tools", "工具调用"),
                yesNo(model.supportsTools),
              ],
              [
                t("platformCatalog.streaming", "流式输出"),
                yesNo(model.supportsStreaming),
              ],
              [
                t("platformCatalog.reasoningLevels", "思考档位"),
                list(model.reasoningLevels),
              ],
              [
                t("platformCatalog.defaultReasoning", "默认思考档位"),
                model.defaultReasoningLevel || unknown,
              ],
            ].map(([label, value]) => (
              <div
                key={label}
                className="min-w-0 space-y-1 rounded-control bg-subtle px-3 py-2.5"
              >
                <dt className="text-fg-3">{label}</dt>
                <dd className="break-words font-medium text-fg-1">{value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
