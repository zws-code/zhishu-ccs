import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  configHistoryApi,
  type ProviderSyncReport,
} from "@/zhishu/config-history/api/configHistory";
import { copyText } from "@/lib/clipboard";
import { toast } from "@/lib/toast";
import { extractErrorMessage } from "@/utils/errorUtils";

export function ProviderSyncReportView({
  id,
  report,
}: {
  id: string;
  report: ProviderSyncReport;
}) {
  const { t } = useTranslation();
  const [opening, setOpening] = useState(false);
  const openBackup = async () => {
    setOpening(true);
    try {
      await configHistoryApi.openSyncBackup(id);
    } catch (error) {
      toast.error(t("platformCatalog.openBackupFailed", "无法打开备份目录"), {
        description: extractErrorMessage(error),
      });
    } finally {
      setOpening(false);
    }
  };
  const copyBackup = async () => {
    if (!report.backupPath) return;
    try {
      await copyText(report.backupPath);
      toast.success(t("platformCatalog.backupPathCopied", "备份路径已复制"));
    } catch {
      toast.error(
        t("settings.configHistory.copyFailed", { defaultValue: "复制失败" }),
      );
    }
  };
  return (
    <div className="space-y-4 overflow-auto text-body">
      <p>
        {t("platformCatalog.syncStats", {
          defaultValue:
            "供应商：新增 {{added}}，更新 {{updated}}，删除 {{removed}}，未变化 {{unchanged}}",
          ...report,
        })}
      </p>
      <section className="space-y-2 rounded-control border border-border p-3">
        <h3 className="font-medium">
          {t("platformCatalog.nativeChanges", "Agent 原生配置文件")}
        </h3>
        {report.changedFiles.length ? (
          <ul className="space-y-1 break-all font-mono text-caption text-fg-2">
            {report.changedFiles.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        ) : (
          <p className="text-fg-2">
            {t(
              "platformCatalog.noNativeChanges",
              "未修改 Agent 原生配置；本次仅同步 CCS 内的供应商列表。",
            )}
          </p>
        )}
      </section>
      <section className="space-y-2 rounded-control border border-border p-3">
        <h3 className="font-medium">
          {t("platformCatalog.syncBackup", "同步前备份")}
        </h3>
        {report.backupPath ? (
          <>
            <code className="block select-text break-all text-caption text-fg-2">
              {report.backupPath}
            </code>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="neutral"
                size="compact"
                disabled={opening}
                onClick={() => void openBackup()}
              >
                <FolderOpen className="h-4 w-4" />
                {t("platformCatalog.openBackup", "打开备份目录")}
              </Button>
              <Button
                variant="quiet"
                size="compact"
                onClick={() => void copyBackup()}
              >
                <Copy className="h-4 w-4" />
                {t("platformCatalog.copyBackupPath", "复制备份路径")}
              </Button>
            </div>
          </>
        ) : (
          <p className="text-fg-2">
            {t(
              "platformCatalog.noSyncBackup",
              "本次没有覆盖冲突，未创建额外的同步备份。",
            )}
          </p>
        )}
      </section>
      {report.warnings.map((warning, index) => (
        <p key={index} role="status" className="text-warning-text">
          {warning}
        </p>
      ))}
    </div>
  );
}
