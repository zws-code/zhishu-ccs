import { useCallback, useEffect, useId, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Copy,
  Undo2,
  Eye,
  EyeOff,
  FileClock,
  Loader2,
  RefreshCw,
  ShieldAlert,
  Trash2,
} from "lucide-react";
import {
  configHistoryApi,
  type ConfigChangeDetail,
  type ConfigChangeFileSummary,
  type ConfigChangeGroup,
  type ConfigOperationSummary,
} from "@/zhishu/config-history/api/configHistory";
import { APP_DISPLAY_NAME, AppGlyph } from "@/components/shell/AppGlyph";
import { APP_IDS } from "@/config/appConfig";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { toast } from "@/lib/toast";
import { extractErrorMessage } from "@/utils/errorUtils";
import type { AppId } from "@/lib/api/types";
import { useQueryClient } from "@tanstack/react-query";
import { useConfigChangeHistory } from "@/zhishu/config-history/query/configHistory";
import { copyText } from "@/lib/clipboard";
import {
  configUnifiedDiff,
  diffConfigText,
} from "@/zhishu/config-history/configHistoryDiff";
import { PageTabs } from "@/components/ui/page-tabs";
import { SessionDiff } from "@/components/sessions/reader/SessionDiff";
import { ProviderSyncReportView } from "@/zhishu/config-history/ProviderSyncReportView";

type AppFilter = "all" | AppId;

function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

export function formatChangeTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

/** 配置历史浏览与还原入口：区分文件差异、供应商同步报告和成员操作摘要，敏感内容按需显示。 */
export function ConfigChangeHistoryPanel({
  app,
  nested = false,
}: {
  app?: AppId;
  nested?: boolean;
}) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const tabsId = useId();
  const [appFilter, setAppFilter] = useState<AppFilter>("all");
  const history = useConfigChangeHistory(
    app ?? (appFilter === "all" ? undefined : appFilter),
  );
  const items = history.data ?? [];
  const loading = history.isPending;
  const load = async () => {
    await history.refetch();
  };
  const [selected, setSelected] = useState<ConfigChangeGroup | null>(null);
  const [selectedFileId, setSelectedFileId] = useState<string | null>(null);
  const [detail, setDetail] = useState<ConfigChangeDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState(false);
  const [detailAttempt, setDetailAttempt] = useState(0);
  const [revealSensitive, setRevealSensitive] = useState(false);
  const [view, setView] = useState<"diff" | "snapshots">("diff");
  const [clearOpen, setClearOpen] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [restoring, setRestoring] = useState(false);

  const selectedFile = useMemo(
    () =>
      selected?.files.find((file) => file.id === selectedFileId) ??
      selected?.files[0] ??
      null,
    [selected, selectedFileId],
  );

  useEffect(() => {
    if (!selected || !selectedFileId) return;
    let active = true;
    setDetailLoading(true);
    setDetailError(false);
    setDetail(null);
    void configHistoryApi
      .get(selectedFileId, revealSensitive)
      .then((value) => {
        if (!active) return;
        if (!value) {
          toast.error(
            t("settings.configHistory.missing", {
              defaultValue: "这条记录已经不存在",
            }),
          );
          setSelected(null);
          void client.invalidateQueries({ queryKey: ["configChangeHistory"] });
        } else setDetail(value);
      })
      .catch((error) => {
        if (!active) return;
        setDetailError(true);
        toast.error(
          t("settings.configHistory.detailFailed", {
            defaultValue: "无法读取修改详情",
          }),
          { description: extractErrorMessage(error) },
        );
      })
      .finally(() => {
        if (active) setDetailLoading(false);
      });
    return () => {
      active = false;
    };
  }, [selected, selectedFileId, revealSensitive, detailAttempt, client, t]);

  const openRecord = (record: ConfigChangeGroup) => {
    setDetail(null);
    setDetailLoading(true);
    setSelected(record);
    setSelectedFileId(record.files[0]?.id ?? null);
    setRevealSensitive(false);
    setView("diff");
  };
  const selectFile = (id: string) => {
    setDetail(null);
    setDetailLoading(true);
    setSelectedFileId(id);
  };
  const toggleSensitive = () => {
    setDetail(null);
    setDetailLoading(true);
    setRevealSensitive((value) => !value);
  };
  const handleCopy = async (value: string) => {
    try {
      await copyText(value);
      toast.success(
        t("settings.configHistory.copied", { defaultValue: "已复制" }),
      );
    } catch (error) {
      toast.error(
        t("settings.configHistory.copyFailed", { defaultValue: "复制失败" }),
        { description: extractErrorMessage(error) },
      );
    }
  };
  const handleRestore = async () => {
    if (!selected || restoring) return;
    setRestoring(true);
    try {
      const ids = selected.files.map((file) => file.id);
      const result = await configHistoryApi.restore(ids);
      setRestoreOpen(false);
      if (result.changed) {
        toast.success(
          t("settings.configHistory.restoredFiles", {
            count: result.files.filter((file) => file.changed).length,
            defaultValue: "已还原 {{count}} 个文件，当前配置已备份",
          }),
          {
            description: result.pointerRestored
              ? t("settings.configHistory.pointerRestored", {
                  defaultValue: "当前供应商已回到这次切换之前",
                })
              : undefined,
          },
        );
      } else {
        toast.success(
          t("settings.configHistory.alreadyRestored", {
            defaultValue: "当前文件已经是修改前的内容",
          }),
        );
      }
      await client.invalidateQueries();
    } catch (error) {
      toast.error(
        t("settings.configHistory.restoreFailed", { defaultValue: "还原失败" }),
        { description: extractErrorMessage(error) },
      );
    } finally {
      setRestoring(false);
    }
  };
  const diff = useMemo(
    () =>
      detail ? configUnifiedDiff(detail.before, detail.after, detail.path) : "",
    [detail],
  );
  const counts = useMemo(() => {
    const rows = detail ? diffConfigText(detail.before, detail.after) : [];
    return {
      added: rows.filter((line) => line.type === "add").length,
      removed: rows.filter((line) => line.type === "del").length,
    };
  }, [detail]);

  const handleClear = useCallback(async () => {
    setClearing(true);
    try {
      const removed = await configHistoryApi.clear();
      setClearOpen(false);
      setSelected(null);
      setSelectedFileId(null);
      setDetail(null);
      await client.invalidateQueries({ queryKey: ["configChangeHistory"] });
      toast.success(
        t("settings.configHistory.cleared", {
          count: removed,
          defaultValue: "已清空 {{count}} 条配置修改记录",
        }),
      );
    } catch (error) {
      toast.error(
        t("settings.configHistory.clearFailed", {
          defaultValue: "清空修改记录失败",
        }),
        { description: extractErrorMessage(error) },
      );
    } finally {
      setClearing(false);
    }
  }, [client, t]);

  const actionLabel = useMemo(
    () => (action: string) => {
      const keys: Record<string, string> = {
        switch: "settings.configHistory.actions.switch",
        apply: "settings.configHistory.actions.apply",
        enter: "settings.configHistory.actions.enter",
        exit: "settings.configHistory.actions.exit",
        detach: "settings.configHistory.actions.detach",
        attach: "settings.configHistory.actions.attach",
        route: "settings.configHistory.actions.route",
        stack: "settings.configHistory.actions.stack",
        catalog: "settings.configHistory.actions.catalog",
        restore: "settings.configHistory.actions.restore",
        add_provider: "settings.configHistory.actions.addProvider",
        remove_provider: "settings.configHistory.actions.removeProvider",
        config_update: "settings.configHistory.actions.configUpdate",
        platform_sync: "platformCatalog.syncAction",
        platform_delete: "platformCatalog.deleteAction",
      };
      return t(keys[action] ?? "settings.configHistory.actions.other", {
        defaultValue: action,
      });
    },
    [t],
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        {!app && (
          <Select
            value={appFilter}
            onValueChange={(value) => setAppFilter(value as AppFilter)}
          >
            <SelectTrigger className="h-9 w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">
                {t("settings.configHistory.allApps", {
                  defaultValue: "全部应用",
                })}
              </SelectItem>
              {APP_IDS.map((app) => (
                <SelectItem key={app} value={app}>
                  {APP_DISPLAY_NAME[app]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        <Button
          variant="neutral"
          size="compact"
          onClick={() => void load()}
          disabled={loading}
          aria-label={t("settings.configHistory.refresh", {
            defaultValue: "刷新",
          })}
        >
          <RefreshCw className={loading ? "animate-spin" : undefined} />
          {t("settings.configHistory.refresh", {
            defaultValue: "刷新",
          })}
        </Button>
        {!app && (
          <Button
            variant="neutral"
            size="compact"
            className="ms-auto text-danger-text"
            onClick={() => setClearOpen(true)}
            disabled={items.length === 0}
          >
            <Trash2 />
            {t("settings.configHistory.clear", {
              defaultValue: "清空记录",
            })}
          </Button>
        )}
      </div>

      {history.isError && (
        <div role="alert" className="text-caption text-danger-text">
          {t("settings.configHistory.loadFailed", {
            defaultValue: "无法读取配置修改记录",
          })}
        </div>
      )}
      <div className="overflow-hidden rounded-panel border border-border bg-surface">
        {loading ? (
          <div className="flex min-h-36 items-center justify-center">
            <Loader2 className="h-5 w-5 animate-spin text-fg-3" />
          </div>
        ) : history.isError && !history.data ? null : items.length === 0 ? (
          <div className="flex min-h-40 flex-col items-center justify-center gap-2 px-6 text-center">
            <FileClock className="h-7 w-7 text-fg-3" />
            <div className="text-body font-medium text-fg-1">
              {t("settings.configHistory.emptyTitle", {
                defaultValue: "还没有配置修改记录",
              })}
            </div>
            <p className="m-0 max-w-md text-caption text-fg-2">
              {t("settings.configHistory.emptyDescription", {
                defaultValue:
                  "以后 zhishu-ccs 写入 Codex、Claude、Gemini 等应用配置时，会在这里自动留下修改前和修改后的内容。",
              })}
            </p>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {items.map((record) => (
              <button
                key={record.id}
                type="button"
                className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-surface-2"
                onClick={() => openRecord(record)}
              >
                <AppGlyph app={record.app} size={18} />
                <div className="min-w-0 flex-1">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate text-body font-medium text-fg-1">
                      {APP_DISPLAY_NAME[record.app]}
                    </span>
                    <span className="shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium text-fg-2 ring-1 ring-border">
                      {actionLabel(record.action)}
                    </span>
                    {record.sensitive && (
                      <ShieldAlert className="h-3.5 w-3.5 shrink-0 text-warning-text" />
                    )}
                  </div>
                  {record.summary && (
                    <OperationSummaryLines summary={record.summary} />
                  )}
                  <div className="mt-0.5 truncate font-mono text-caption text-fg-2">
                    {record.sync ? (
                      <>
                        {t("platformCatalog.syncResult", "供应商同步结果")}
                        <span className="ms-2 font-sans text-fg-3">
                          {t("platformCatalog.syncStats", {
                            defaultValue:
                              "供应商：新增 {{added}}，更新 {{updated}}，删除 {{removed}}，未变化 {{unchanged}}",
                            ...record.sync,
                          })}
                        </span>
                      </>
                    ) : record.files.length > 1 ? (
                      <TooltipProvider delayDuration={0} skipDelayDuration={0}>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <span className="block truncate">
                              {t("settings.configHistory.changedFiles", {
                                count: record.files.length,
                                defaultValue: "修改了 {{count}} 个文件",
                              })}
                              <span className="ms-2 font-sans text-fg-3">
                                {record.files
                                  .map((file) => fileName(file.path))
                                  .join("、")}
                              </span>
                            </span>
                          </TooltipTrigger>
                          <TooltipContent
                            side="top"
                            align="start"
                            className="max-w-[480px] whitespace-pre-wrap break-all rounded-panel text-left font-mono leading-5"
                          >
                            {record.files.map((file) => file.path).join("\n")}
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                    ) : (
                      <>
                        {fileName(record.files[0].path)}
                        <span className="ms-2 font-sans text-fg-3">
                          {record.files[0].path}
                        </span>
                      </>
                    )}
                  </div>
                </div>
                <div className="shrink-0 text-right text-caption text-fg-2">
                  <div>{formatChangeTime(record.changedAt)}</div>
                  <div className="mt-0.5 text-fg-3">
                    {record.sync
                      ? record.sync.changedFiles.length
                        ? t("platformCatalog.nativeChangedCount", {
                            defaultValue: "{{count}} 个原生配置文件变更",
                            count: record.sync.changedFiles.length,
                          })
                        : t(
                            "platformCatalog.noNativeChangesShort",
                            "未修改原生配置文件",
                          )
                      : record.summary &&
                          record.files.every(
                            (file) => !file.beforeExists && !file.afterExists,
                          )
                        ? t("settings.configHistory.viewSummary", {
                            defaultValue: "查看详情",
                          })
                        : record.files.length > 1
                          ? t("settings.configHistory.viewChanges", {
                              defaultValue: "查看前后内容",
                            })
                          : !record.files[0].beforeExists
                            ? t("settings.configHistory.fileCreated", {
                                defaultValue: "新建文件",
                              })
                            : !record.files[0].afterExists
                              ? t("settings.configHistory.fileDeleted", {
                                  defaultValue: "删除文件",
                                })
                              : t("settings.configHistory.viewChanges", {
                                  defaultValue: "查看前后内容",
                                })}
                  </div>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      <Dialog
        open={!!selected}
        onOpenChange={(open) => {
          if (!open && !restoring && !restoreOpen) {
            setSelected(null);
            setSelectedFileId(null);
            setDetail(null);
            setRevealSensitive(false);
          }
        }}
      >
        <DialogContent
          zIndex={nested ? "nested" : "base"}
          className="w-[calc(100vw-2rem)] max-w-[1180px]"
        >
          <DialogHeader>
            <DialogTitle>
              {selected ? APP_DISPLAY_NAME[selected.app] : ""}
              {selected
                ? ` · ${
                    selected.sync
                      ? t("platformCatalog.syncResult", "供应商同步结果")
                      : selected.summary
                        ? actionLabel(selected.action)
                        : selected.files.length > 1
                          ? t("settings.configHistory.changedFiles", {
                              count: selected.files.length,
                              defaultValue: "修改了 {{count}} 个文件",
                            })
                          : fileName(selected.files[0].path)
                  }`
                : ""}
            </DialogTitle>
            <DialogDescription className="break-all font-mono">
              {selected?.sync
                ? t(
                    "platformCatalog.syncResultDescription",
                    "查看本次同步结果、原生配置文件变化与备份位置。",
                  )
                : selectedFile?.path}
            </DialogDescription>
          </DialogHeader>
          <div className="flex h-[60vh] min-h-0 flex-col gap-3 overflow-hidden px-6 pb-5">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-caption text-fg-2">
                {selected ? formatChangeTime(selected.changedAt) : ""}
              </span>
              {selected && (
                <span className="rounded px-1.5 py-0.5 text-[11px] font-medium text-fg-2 ring-1 ring-border">
                  {actionLabel(selected.action)}
                </span>
              )}
              {selected && !selected.sync && selected.files.length > 1 && (
                <Select
                  value={selectedFileId ?? undefined}
                  onValueChange={selectFile}
                >
                  <SelectTrigger
                    className="h-8 w-56"
                    aria-label={t("settings.configHistory.selectFile", {
                      defaultValue: "选择文件",
                    })}
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {selected.files.map((file) => (
                      <SelectItem
                        key={file.id}
                        value={file.id}
                        title={file.path}
                      >
                        {fileName(file.path)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
              {detail?.sensitive && (
                <Button
                  variant="neutral"
                  size="compact"
                  className="ms-auto"
                  onClick={toggleSensitive}
                  disabled={detailLoading || restoring}
                >
                  {revealSensitive ? <EyeOff /> : <Eye />}
                  {revealSensitive
                    ? t("settings.configHistory.hideSensitive", {
                        defaultValue: "隐藏敏感值",
                      })
                    : t("settings.configHistory.showSensitive", {
                        defaultValue: "显示敏感值",
                      })}
                </Button>
              )}
            </div>
            {detailLoading ? (
              <div className="flex flex-1 items-center justify-center">
                <Loader2 className="h-5 w-5 animate-spin text-fg-3" />
              </div>
            ) : detail?.sync ? (
              <div className="min-h-0 flex-1 overflow-auto">
                <ProviderSyncReportView id={detail.id} report={detail.sync} />
              </div>
            ) : detail?.summary ? (
              <div className="min-h-0 flex-1 overflow-auto">
                <OperationSummaryView
                  summary={detail.summary}
                  files={selected?.files ?? []}
                />
              </div>
            ) : detail ? (
              <>
                <div className="flex flex-wrap items-center gap-2">
                  <PageTabs
                    aria-label={t("settings.configHistory.viewMode")}
                    idPrefix={tabsId}
                    controls={`${tabsId}-content`}
                    value={view}
                    onValueChange={setView}
                    size="sm"
                    items={[
                      {
                        value: "diff",
                        label: t("settings.configHistory.diff", {
                          defaultValue: "差异对比",
                        }),
                      },
                      {
                        value: "snapshots",
                        label: t("settings.configHistory.snapshots", {
                          defaultValue: "完整内容",
                        }),
                      },
                    ]}
                  />
                  <span className="text-caption text-fg-2">
                    {t("settings.configHistory.changeCount", {
                      added: counts.added,
                      removed: counts.removed,
                      defaultValue: "新增 {{added}} 行 · 删除 {{removed}} 行",
                    })}
                  </span>
                  <Button
                    className="ms-auto"
                    variant="neutral"
                    size="compact"
                    onClick={() => setRestoreOpen(true)}
                    disabled={restoring}
                  >
                    <Undo2 />
                    {t("settings.configHistory.restore", {
                      defaultValue: "还原到修改前",
                    })}
                  </Button>
                </div>
                {detail.sensitive && !revealSensitive && (
                  <p className="m-0 text-caption text-warning-text">
                    {t("settings.configHistory.maskedHint", {
                      defaultValue:
                        "敏感值已隐藏。对比和复制仅包含当前显示的内容；显示敏感值后可查看完整差异。",
                    })}
                  </p>
                )}
                <div
                  role="tabpanel"
                  id={`${tabsId}-content`}
                  aria-labelledby={`${tabsId}-${view}`}
                  className="min-h-0 flex-1 overflow-auto"
                >
                  {view === "diff" ? (
                    <div className="space-y-2">
                      <Button
                        variant="neutral"
                        size="compact"
                        onClick={() => void handleCopy(diff)}
                        disabled={!diff}
                      >
                        <Copy />
                        {t("settings.configHistory.copyDiff", {
                          defaultValue: "复制差异",
                        })}
                      </Button>
                      {detail.before === null || detail.after === null ? (
                        <p className="m-0 text-caption text-fg-2">
                          {t(
                            detail.before === null
                              ? "settings.configHistory.fileCreated"
                              : "settings.configHistory.fileDeleted",
                          )}
                        </p>
                      ) : null}
                      {diff ? (
                        <SessionDiff text={diff} className="max-h-none" />
                      ) : (
                        <p className="py-8 text-center text-caption text-fg-3">
                          {t("settings.configHistory.noVisibleDiff", {
                            defaultValue: "当前显示的内容没有差异",
                          })}
                        </p>
                      )}
                    </div>
                  ) : (
                    <div className="grid min-h-0 gap-3 md:grid-cols-2">
                      <ConfigSnapshot
                        title={t("settings.configHistory.before", {
                          defaultValue: "修改前",
                        })}
                        value={detail.before}
                        emptyLabel={t(
                          "settings.configHistory.fileDidNotExist",
                          { defaultValue: "文件不存在" },
                        )}
                        onCopy={handleCopy}
                      />
                      <ConfigSnapshot
                        title={t("settings.configHistory.after", {
                          defaultValue: "修改后",
                        })}
                        value={detail.after}
                        emptyLabel={t("settings.configHistory.fileDeleted", {
                          defaultValue: "文件已删除",
                        })}
                        onCopy={handleCopy}
                      />
                    </div>
                  )}
                </div>
              </>
            ) : detailError ? (
              <Button
                variant="neutral"
                onClick={() => setDetailAttempt((value) => value + 1)}
              >
                {t("settings.configHistory.refresh", { defaultValue: "刷新" })}
              </Button>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              variant="neutral"
              onClick={() => {
                setSelected(null);
                setSelectedFileId(null);
                setDetail(null);
                setRevealSensitive(false);
              }}
              disabled={restoring}
            >
              {t("common.close", { defaultValue: "关闭" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={restoreOpen}
        onOpenChange={(open) => {
          if (!restoring) setRestoreOpen(open);
        }}
      >
        <DialogContent
          zIndex="alert"
          className="max-w-lg"
          onEscapeKeyDown={(event) => {
            if (restoring) event.preventDefault();
          }}
        >
          <DialogHeader>
            <DialogTitle>
              {t("settings.configHistory.restoreTitle", {
                defaultValue: "还原到这次修改之前？",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("settings.configHistory.restoreDescription", {
                defaultValue:
                  "还原这次操作修改的全部配置文件，不更改供应商列表。若还原的是切换供应商，当前选择也会回到切换之前。会先备份当前文件；若任一文件已有后续修改，将停止还原。",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="px-6 text-caption text-fg-2">
            {selected && selected.files.length > 1 && (
              <div className="mb-1">
                {t("settings.configHistory.restoreFilesIntro", {
                  count: selected.files.length,
                  defaultValue: "将还原 {{count}} 个文件：",
                })}
              </div>
            )}
            <ul className="m-0 list-none break-all p-0 font-mono">
              {selected?.files.map((file) => (
                <li key={file.id}>{file.path}</li>
              ))}
            </ul>
            {selected && formatChangeTime(selected.changedAt)}
            {selected?.files.some((file) => !file.beforeExists) && (
              <p className="text-danger-text">
                {t("settings.configHistory.restoreDeletesFile", {
                  defaultValue: "修改前文件不存在，还原将删除这些新建文件。",
                })}
              </p>
            )}
          </div>
          <DialogFooter>
            <Button
              variant="neutral"
              onClick={() => setRestoreOpen(false)}
              disabled={restoring}
            >
              {t("common.cancel", { defaultValue: "取消" })}
            </Button>
            <Button
              variant="solid"
              onClick={() => void handleRestore()}
              disabled={restoring}
            >
              {restoring ? <Loader2 className="animate-spin" /> : <Undo2 />}
              {t("settings.configHistory.restore", {
                defaultValue: "还原到修改前",
              })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={clearOpen} onOpenChange={setClearOpen}>
        <DialogContent zIndex="alert" className="max-w-md">
          <DialogHeader>
            <DialogTitle>
              {t("settings.configHistory.clearTitle", {
                defaultValue: "清空配置修改记录？",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("settings.configHistory.clearDescription", {
                defaultValue:
                  "这会永久删除本机上的修改前后快照。应用配置本身不会改变。",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="neutral"
              onClick={() => setClearOpen(false)}
              disabled={clearing}
            >
              {t("common.cancel", { defaultValue: "取消" })}
            </Button>
            <Button
              variant="solid"
              onClick={() => void handleClear()}
              disabled={clearing}
            >
              {clearing && <Loader2 className="animate-spin" />}
              {t("settings.configHistory.clear", {
                defaultValue: "清空记录",
              })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/** One line per roster change (added/removed members) shown in the list row. */
function OperationSummaryLines({
  summary,
}: {
  summary: ConfigOperationSummary;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-0.5 space-y-0.5 text-body text-fg-1">
      {summary.added.length > 0 && (
        <p className="m-0">
          {t("settings.configHistory.summaryAdded", {
            names: summary.added.join("、"),
            defaultValue: "添加了 {{names}}",
          })}
        </p>
      )}
      {summary.removed.length > 0 && (
        <p className="m-0">
          {t("settings.configHistory.summaryRemoved", {
            names: summary.removed.join("、"),
            defaultValue: "移除了 {{names}}",
          })}
        </p>
      )}
    </div>
  );
}

/**
 * Detail view for a roster-style summary record: it has no before/after
 * snapshots, so there is no diff and nothing to restore.
 */
function OperationSummaryView({
  summary,
  files,
}: {
  summary: ConfigOperationSummary;
  files: ConfigChangeFileSummary[];
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4 text-body">
      {summary.added.length > 0 && (
        <p className="m-0">
          {t("settings.configHistory.summaryAdded", {
            names: summary.added.join("、"),
            defaultValue: "添加了 {{names}}",
          })}
        </p>
      )}
      {summary.removed.length > 0 && (
        <p className="m-0">
          {t("settings.configHistory.summaryRemoved", {
            names: summary.removed.join("、"),
            defaultValue: "移除了 {{names}}",
          })}
        </p>
      )}
      <section className="space-y-2 rounded-control border border-border p-3">
        <h3 className="m-0 font-medium">
          {t("settings.configHistory.summaryFiles", {
            defaultValue: "涉及文件",
          })}
        </h3>
        {files.length > 0 ? (
          <ul className="m-0 list-none space-y-1 break-all p-0 font-mono text-caption text-fg-2">
            {files.map((file) => (
              <li key={file.id}>{file.path}</li>
            ))}
          </ul>
        ) : (
          <p className="m-0 text-caption text-fg-2">
            {t("settings.configHistory.summaryNoFiles", {
              defaultValue: "本次没有修改任何配置文件。",
            })}
          </p>
        )}
      </section>
    </div>
  );
}

function ConfigSnapshot({
  title,
  value,
  emptyLabel,
  onCopy,
}: {
  title: string;
  value: string | null;
  emptyLabel: string;
  onCopy: (value: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  return (
    <section className="flex min-h-0 flex-col overflow-hidden rounded-panel border border-border bg-bg">
      <div className="flex items-center justify-between border-b border-border bg-surface px-3 py-2 text-caption font-medium text-fg-1">
        {title}
        <Button
          variant="quiet"
          size="compact"
          disabled={value === null}
          onClick={() => value !== null && void onCopy(value)}
          aria-label={t("settings.configHistory.copySnapshot", {
            title,
            defaultValue: "复制{{title}}",
          })}
        >
          <Copy />
          {t("common.copy", { defaultValue: "复制" })}
        </Button>
      </div>
      {value === null ? (
        <div className="flex min-h-56 items-center justify-center px-4 text-caption text-fg-3">
          {emptyLabel}
        </div>
      ) : (
        <pre className="m-0 min-h-56 overflow-auto whitespace-pre-wrap break-words p-3 font-mono text-xs leading-5 text-fg-1">
          {value || " "}
        </pre>
      )}
    </section>
  );
}
