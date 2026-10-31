import { useState } from "react";
import { useTranslation } from "react-i18next";
import { FileClock } from "lucide-react";
import type { AppId } from "@/lib/api/types";
import { useConfigChangeHistory } from "@/zhishu/config-history/query/configHistory";
import { APP_DISPLAY_NAME } from "@/components/shell/AppGlyph";
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
  ConfigChangeHistoryPanel,
  formatChangeTime,
} from "@/zhishu/config-history/ConfigChangeHistoryPanel";

export function AppConfigHistoryButton({ app }: { app: AppId }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const history = useConfigChangeHistory(app);
  const latest = history.data?.[0]?.changedAt;
  const status = latest
    ? t("settings.configHistory.latest", {
        time: formatChangeTime(latest),
        defaultValue: "最近修改：{{time}}",
      })
    : history.isPending
      ? t("common.loading", { defaultValue: "加载中…" })
      : history.isError
        ? t("settings.configHistory.loadFailed", {
            defaultValue: "无法读取配置修改记录",
          })
        : t("settings.configHistory.noChanges", {
            defaultValue: "暂无修改记录",
          });
  return (
    <>
      <Button
        variant="quiet"
        size="compact"
        className="h-auto min-h-8 shrink-0"
        onClick={() => setOpen(true)}
        title={status}
      >
        <FileClock className="h-4 w-4" />
        <span className="text-start">
          <span className="block text-caption">
            {t("settings.configHistory.title", {
              defaultValue: "配置修改记录",
            })}
          </span>
          <span className="block max-w-40 truncate text-[11px] leading-3 tabular-nums text-fg-3">
            {latest ? (
              <time dateTime={latest} aria-label={status}>
                {formatChangeTime(latest)}
              </time>
            ) : (
              status
            )}
          </span>
        </span>
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="w-[calc(100vw-2rem)] max-w-[1080px]">
          <DialogHeader>
            <DialogTitle>
              {APP_DISPLAY_NAME[app]} ·{" "}
              {t("settings.configHistory.title", {
                defaultValue: "配置修改记录",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("settings.configHistory.description")}
            </DialogDescription>
          </DialogHeader>
          <div className="min-h-0 overflow-auto px-6 pb-2">
            <ConfigChangeHistoryPanel key={app} app={app} nested />
          </div>
          <DialogFooter>
            <Button variant="neutral" onClick={() => setOpen(false)}>
              {t("common.close", { defaultValue: "关闭" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
