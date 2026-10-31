import { AlertTriangle, CheckCircle2, UserRound, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import type { PlatformAccount } from "@/zhishu/api/platformAuth";
import type { PlatformDeleteSummary } from "@/zhishu/api/platformModels";
import { PlatformProviderDeletionResult } from "./PlatformProviderDeletionResult";

/** 全应用删除的瞬时反馈；关闭即清除结果，不在目录页面留常驻内容。 */
export function PlatformProviderDeletionDialog({
  account,
  result,
  error,
  onDismiss,
}: {
  account: PlatformAccount;
  result?: PlatformDeleteSummary;
  error: string | null;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const partial = Boolean(
    result &&
      (result.skipped ||
        result.warnings.length ||
        result.apps?.some(
          (app) => app.error || app.skipped || app.warnings.length,
        )),
  );
  const ResultIcon = error || partial ? AlertTriangle : CheckCircle2;
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onDismiss();
      }}
    >
      <DialogContent className="w-[calc(100vw-2rem)] max-w-[440px] overflow-hidden">
        <DialogClose asChild>
          <button
            type="button"
            aria-label={t("platformCleanup.dismiss")}
            className="absolute right-4 top-4 rounded-control p-1 text-fg-3 transition-colors hover:bg-subtle hover:text-fg-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X className="h-4 w-4" />
          </button>
        </DialogClose>
        <DialogHeader className="gap-3 pb-4 pt-6">
          <div
            className={`flex h-10 w-10 items-center justify-center rounded-full ${error || partial ? "bg-warning-soft text-warning-text" : "bg-success-soft text-success-text"}`}
          >
            <ResultIcon className="h-5 w-5" />
          </div>
          <DialogTitle>{t("platformDeletion.result")}</DialogTitle>
          <DialogDescription role="status" className="leading-relaxed">
            {error
              ? t("platformDeletion.failed")
              : result &&
                t("platformCatalog.deleteCompleted", {
                  removed: result.removed,
                  skipped: result.skipped,
                })}
          </DialogDescription>
        </DialogHeader>
        <div className="min-h-0 space-y-4 overflow-y-auto px-6 pb-5">
          <div className="flex items-center gap-3 rounded-control border border-border bg-subtle px-3 py-3">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-surface text-fg-2">
              <UserRound className="h-4 w-4" />
            </div>
            <div className="min-w-0 space-y-0.5">
              <p className="break-all text-body font-medium">
                {account.user.email}
              </p>
              <p className="break-all text-caption text-fg-3">
                {account.baseUrl} · ID {account.user.id}
              </p>
            </div>
          </div>
          {error && (
            <p
              role="alert"
              className="break-words rounded-control bg-warning-soft px-3 py-2.5 text-caption leading-relaxed text-warning-text"
            >
              {error}
            </p>
          )}
          {partial && (
            <div className="space-y-1 rounded-control bg-warning-soft px-3 py-2.5 text-caption leading-relaxed text-warning-text">
              <p>{t("platformDeletion.partial")}</p>
              {result?.warnings.map((warning, index) => (
                <p key={index} className="break-words">
                  {warning}
                </p>
              ))}
            </div>
          )}
          {result && (
            <PlatformProviderDeletionResult
              result={result}
              defaultOpen={false}
            />
          )}
          <p className="text-caption leading-relaxed text-fg-3">
            {t("platformDeletion.retained")}
          </p>
        </div>
        <DialogFooter className="border-t border-border py-4">
          <Button onClick={onDismiss}>{t("common.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
