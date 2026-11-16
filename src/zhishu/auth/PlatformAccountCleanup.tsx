import { useState } from "react";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronRight,
  Trash2,
  UserRound,
  X,
} from "lucide-react";
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
import { usePlatformAuth } from "./PlatformAuthContext";
import {
  platformAuthError,
  type PlatformCleanupReport,
  type PlatformCleanupStep,
} from "@/zhishu/api/platformAuth";
import { PlatformProviderDeletionResult } from "@/zhishu/models/components/PlatformProviderDeletionResult";

const dialogClassName = "w-[calc(100vw-2rem)] max-w-[440px] overflow-hidden";
const bodyClassName = "min-h-0 space-y-4 overflow-y-auto px-6 pb-5";
const footerClassName = "border-t border-border py-4";

function CloseButton() {
  const { t } = useTranslation();
  return (
    <DialogClose asChild>
      <button
        type="button"
        aria-label={t("platformCleanup.dismiss")}
        className="absolute right-4 top-4 rounded-control p-1 text-fg-3 transition-colors hover:bg-subtle hover:text-fg-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <X className="h-4 w-4" />
      </button>
    </DialogClose>
  );
}

function AccountIdentity({
  email,
  baseUrl,
  userId,
}: {
  email: string;
  baseUrl: string;
  userId: number;
}) {
  return (
    <div className="flex items-center gap-3 rounded-control border border-border bg-subtle px-3 py-3">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-surface text-fg-2">
        <UserRound className="h-4 w-4" />
      </div>
      <div className="min-w-0 space-y-0.5">
        <p className="break-all text-body font-medium text-fg-1">{email}</p>
        <p className="break-all text-caption text-fg-3">
          {baseUrl} · ID {userId}
        </p>
      </div>
    </div>
  );
}

export function PlatformAccountCleanupButton({
  disabled = false,
}: {
  disabled?: boolean;
}) {
  const auth = usePlatformAuth();
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const account = auth?.account;
  if (!account) return null;
  return (
    <>
      <button
        type="button"
        className="rounded-control p-1 text-fg-2 hover:bg-subtle hover:text-danger"
        title={t("platformCleanup.action")}
        aria-label={t("platformCleanup.action")}
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <Trash2 className="h-4 w-4" />
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent role="alertdialog" className={dialogClassName}>
          <CloseButton />
          <DialogHeader className="gap-3 pb-4 pt-6">
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-danger-soft text-danger">
              <Trash2 className="h-5 w-5" />
            </div>
            <DialogTitle className="pr-4">
              {t("platformCleanup.title")}
            </DialogTitle>
            <DialogDescription className="leading-relaxed">
              {t("platformCleanup.scope")}
            </DialogDescription>
          </DialogHeader>
          <div className={bodyClassName}>
            <AccountIdentity
              email={account.user.email}
              baseUrl={account.baseUrl}
              userId={account.user.id}
            />
            <div className="space-y-2">
              <p className="flex items-start gap-2 text-caption leading-relaxed text-fg-2">
                <Check className="mt-0.5 h-4 w-4 shrink-0 text-success" />
                {t("platformCleanup.retained")}
              </p>
              <details className="group text-caption text-fg-3">
                <summary className="flex cursor-pointer list-none items-center gap-1 py-1 hover:text-fg-1 [&::-webkit-details-marker]:hidden">
                  <ChevronRight className="h-3.5 w-3.5 transition-transform group-open:rotate-90" />
                  {t("platformCleanup.scopeDetails")}
                </summary>
                <p className="pt-1 leading-relaxed">
                  {t("platformCleanup.retainedDetails")}
                </p>
              </details>
            </div>
            <p className="rounded-control bg-warning-soft px-3 py-2.5 text-caption leading-relaxed text-warning-text">
              {t("platformCleanup.partialHint")}
            </p>
          </div>
          <DialogFooter className={footerClassName}>
            <Button variant="outline" autoFocus onClick={() => setOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                setOpen(false);
                void auth.logoutAndClear();
              }}
            >
              {t("platformCleanup.confirm")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function CleanupStepRow({
  label,
  step,
}: {
  label: string;
  step: PlatformCleanupStep;
}) {
  const { t } = useTranslation();
  return (
    <li className="py-2.5">
      <div className="flex items-center justify-between gap-3">
        <span className="text-fg-2">{label}</span>
        <span
          className={`flex shrink-0 items-center gap-1.5 ${step.completed ? "text-success-text" : "text-warning-text"}`}
        >
          {step.completed ? (
            <Check className="h-3.5 w-3.5" />
          ) : (
            <AlertTriangle className="h-3.5 w-3.5" />
          )}
          {t(
            step.completed ? "platformCleanup.done" : "platformCleanup.failed",
          )}
        </span>
      </div>
      {step.error && (
        <p className="mt-1 break-words text-danger">
          {platformAuthError(step.error)}
        </p>
      )}
    </li>
  );
}

/** 结果通过门禁外的弹窗展示，仅正文滚动，关闭操作始终可见。 */
export function PlatformAccountCleanupResult({
  result,
  onDismiss,
}: {
  result: PlatformCleanupReport;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const steps = [
    ["catalog", result.catalog],
    ["session", result.session],
    ["serverLogout", result.serverLogout],
  ] as const;
  const partial =
    steps.some(([, step]) => !step.completed) ||
    !result.providers ||
    result.providers.skipped > 0 ||
    result.providers.warnings.length > 0 ||
    result.providers.apps?.some((app) => app.error);
  const ResultIcon = partial ? AlertTriangle : CheckCircle2;
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onDismiss();
      }}
    >
      <DialogContent className={dialogClassName}>
        <CloseButton />
        <DialogHeader className="gap-3 pb-4 pt-6">
          <div
            className={`flex h-10 w-10 items-center justify-center rounded-full ${partial ? "bg-warning-soft text-warning-text" : "bg-success-soft text-success-text"}`}
          >
            <ResultIcon className="h-5 w-5" />
          </div>
          <DialogTitle>{t("platformCleanup.result")}</DialogTitle>
          <DialogDescription role="status" className="leading-relaxed">
            {t(
              partial ? "platformCleanup.partial" : "platformCleanup.complete",
            )}
          </DialogDescription>
        </DialogHeader>
        <div className={bodyClassName}>
          <AccountIdentity
            email={result.email}
            baseUrl={result.baseUrl}
            userId={result.userId}
          />
          <ul className="divide-y divide-border text-caption">
            {result.providers && (
              <li className="flex items-center justify-between gap-3 py-2.5">
                <span className="text-fg-2">
                  {t("platformCleanup.providers")}
                </span>
                <span className="text-right font-medium">
                  {t("platformCleanup.providerCounts", {
                    removed: result.providers.removed,
                    skipped: result.providers.skipped,
                  })}
                </span>
              </li>
            )}
            {steps.map(([name, step]) => (
              <CleanupStepRow
                key={name}
                label={t(`platformCleanup.${name}`)}
                step={step}
              />
            ))}
          </ul>
          {result.providerError && (
            <p
              role="alert"
              className="rounded-control bg-warning-soft px-3 py-2.5 text-caption text-warning-text"
            >
              {t("platformCleanup.providers")}：
              {platformAuthError(result.providerError)}
            </p>
          )}
          {!!result.providers?.warnings.length && (
            <div className="space-y-1 rounded-control bg-warning-soft px-3 py-2.5 text-caption leading-relaxed text-warning-text">
              {result.providers.warnings.map((warning, index) => (
                <p key={index} className="break-words">
                  {warning}
                </p>
              ))}
            </div>
          )}
          {result.providers && (
            <PlatformProviderDeletionResult
              result={result.providers}
              defaultOpen={false}
            />
          )}
          <p className="text-caption leading-relaxed text-fg-3">
            {t("platformCleanup.retained")}
          </p>
          {partial && (
            <p className="text-caption leading-relaxed text-fg-2">
              {t("platformCleanup.retryHint")}
            </p>
          )}
        </div>
        <DialogFooter className={footerClassName}>
          <Button onClick={onDismiss}>{t("common.close")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
