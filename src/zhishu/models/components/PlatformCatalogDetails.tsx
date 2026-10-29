import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import type {
  PlatformCatalogGroup,
  PlatformCatalogModel,
} from "@/zhishu/api/platformModels";
import { PlatformCatalogKey } from "@/zhishu/models/components/PlatformCatalogKey";

/** 展示已有分组摘要与模型参数；打开详情不触发同步，凭据由独立组件按需读取。 */
export function PlatformCatalogDetails({
  group,
  baseUrl,
  userId,
  syncedAt,
}: {
  group: PlatformCatalogGroup;
  baseUrl: string;
  userId: number;
  syncedAt: number;
}) {
  const { t, i18n } = useTranslation();
  const unknown = t("platformCatalog.notReported", "未提供");
  const status = group.failed
    ? t("platformCatalog.failed", "同步失败，可重试")
    : t("platformCatalog.ready", "已同步");
  const billing =
    group.subscriptionType === "subscription"
      ? t("platformCatalog.subscription", "订阅")
      : group.subscriptionType === "standard"
        ? t("platformCatalog.standard", "按量计费")
        : group.subscriptionType || unknown;

  return (
    <SheetContent
      width={560}
      closeLabel={t("common.close", "关闭")}
      dismissOnOutsideClick
    >
      <SheetHeader>
        <SheetTitle className="break-words">{group.name}</SheetTitle>
        <SheetDescription>
          {t("platformCatalog.detailsDescription", "分组信息与模型参数")}
        </SheetDescription>
      </SheetHeader>
      <SheetBody className="space-y-6">
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-3 text-body">
          {[
            [t("platformCatalog.groupId", "分组 ID"), String(group.id)],
            [
              t("platformCatalog.platform", "平台类型"),
              group.platform || unknown,
            ],
            [t("platformCatalog.billing", "计费类型"), billing],
            [
              t("platformCatalog.groupStatus", "分组状态"),
              group.status || unknown,
            ],
            [t("platformCatalog.userId", "用户 ID"), String(userId)],
            [t("platformCatalog.source", "平台地址"), baseUrl],
            [
              t("platformCatalog.lastSync", "上次同步"),
              new Date(syncedAt).toLocaleString(
                i18n.resolvedLanguage ?? i18n.language,
              ),
            ],
            [t("platformCatalog.status", "同步状态"), status],
          ].map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-fg-3">{label}</dt>
              <dd className="break-words text-fg-1">{value}</dd>
            </div>
          ))}
        </dl>
        {group.error && (
          <p role="status" className="text-body text-danger">
            {group.error}
          </p>
        )}
        <section
          aria-label="API Key"
          className="space-y-3 border-t border-border pt-4"
        >
          <h3 className="text-section">API Key</h3>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-2 text-caption">
            <dt className="text-fg-3">
              {t("platformCatalog.keyId", "Key ID")}
            </dt>
            <dd>{group.keyId ?? unknown}</dd>
            <dt className="text-fg-3">
              {t("platformCatalog.keyName", "Key 名称")}
            </dt>
            <dd className="break-all">{group.keyName || unknown}</dd>
          </dl>
          <PlatformCatalogKey
            key={`${baseUrl}:${userId}:${group.id}:${syncedAt}`}
            baseUrl={baseUrl}
            userId={userId}
            groupId={group.id}
            available={group.keyAvailable !== false}
          />
        </section>
        <ModelCatalog
          key={`${baseUrl}:${userId}:${group.id}:${syncedAt}`}
          models={group.models}
        />
      </SheetBody>
    </SheetContent>
  );
}

function ModelCatalog({ models }: { models: PlatformCatalogModel[] }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="border-t border-border pt-4"
    >
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-section">
          {t("platformCatalog.modelCatalog", "模型目录")} · {models.length}
        </h3>
        <CollapsibleTrigger asChild>
          <Button variant="quiet" size="compact" disabled={models.length === 0}>
            {open ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
            {open
              ? t("platformCatalog.collapseCatalog", "收起模型目录")
              : t("platformCatalog.expandCatalog", "展开模型目录")}
          </Button>
        </CollapsibleTrigger>
      </div>
      {models.length === 0 && (
        <p className="mt-2 text-body text-fg-3">
          {t("platformCatalog.noModels", "暂无可展示的模型数据")}
        </p>
      )}
      <CollapsibleContent className="mt-3 space-y-2">
        {models.map((model) => (
          <ModelDetails key={model.id} model={model} />
        ))}
      </CollapsibleContent>
    </Collapsible>
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
      className="rounded-control border border-border px-3 py-2"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <h4 className="break-words text-body font-medium">{model.name}</h4>
          <code className="block break-all text-caption text-fg-3">
            {model.id}
          </code>
        </div>
        <CollapsibleTrigger asChild>
          <Button
            variant="quiet"
            size="compact"
            aria-label={t("platformCatalog.toggleModel", {
              defaultValue: "{{name}} 的模型参数",
              name: model.name,
            })}
          >
            {open ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
            {open
              ? t("platformCatalog.collapse", "收起")
              : t("platformCatalog.expand", "展开")}
          </Button>
        </CollapsibleTrigger>
      </div>
      <CollapsibleContent>
        {model.description && (
          <p className="mt-3 whitespace-pre-wrap break-words text-body text-fg-2">
            {model.description}
          </p>
        )}
        <dl className="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-caption">
          {[
            [
              t("platformCatalog.contextWindow", "上下文窗口"),
              number(model.contextWindow),
            ],
            [
              t("platformCatalog.maxOutput", "最大输出 Token"),
              number(model.maxOutputTokens),
            ],
            [t("platformCatalog.endpoints", "接口协议"), list(model.endpoints)],
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
            <div key={label} className="contents">
              <dt className="text-fg-3">{label}</dt>
              <dd className="break-words text-fg-2">{value}</dd>
            </div>
          ))}
        </dl>
      </CollapsibleContent>
    </Collapsible>
  );
}
