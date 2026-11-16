import { useTranslation } from "react-i18next";
import { getAppLabel } from "@/config/appConfig";
import type { PlatformDeleteSummary } from "@/zhishu/api/platformModels";
import { platformAuthError } from "@/zhishu/api/platformAuth";

export function PlatformProviderDeletionResult({
  result,
  defaultOpen = true,
}: {
  result: PlatformDeleteSummary;
  defaultOpen?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <details
      open={defaultOpen}
      className="mb-4 rounded-control border border-border bg-surface text-caption"
    >
      <summary className="cursor-pointer px-3 py-2 font-medium text-fg-1">
        {t("platformCatalog.deleteAllResult", "所有 Agent 应用删除结果")}
      </summary>
      <p role="status" className="px-3 pb-2 text-fg-2">
        {t("platformCatalog.deleteCompleted", {
          defaultValue: "已删除 {{removed}} 个平台供应商，保留 {{skipped}} 个",
          removed: result.removed,
          skipped: result.skipped,
        })}
      </p>
      <table
        className="w-full text-start"
        aria-label={t(
          "platformCatalog.deleteAllResult",
          "所有 Agent 应用删除结果",
        )}
      >
        <thead className="bg-subtle text-fg-3">
          <tr>
            <th scope="col" className="px-3 py-2 text-start font-medium">
              {t("platformCatalog.deleteResultApp", "Agent 应用")}
            </th>
            <th scope="col" className="px-3 py-2 text-start font-medium">
              {t("platformCatalog.deleteResultStatus", "删除结果")}
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {result.apps?.map((app) => (
            <tr key={app.agent}>
              <th
                scope="row"
                className="px-3 py-2 text-start align-top font-medium"
              >
                {getAppLabel(app.agent)}
              </th>
              <td className="px-3 py-2 text-fg-2">
                {app.error ? (
                  <p className="text-danger">
                    {t("platformCatalog.deleteAppFailed", {
                      defaultValue: "删除失败：{{message}}",
                      message: platformAuthError(app.error),
                    })}
                  </p>
                ) : (
                  <p>
                    {t("platformCatalog.deleteCompleted", {
                      defaultValue:
                        "已删除 {{removed}} 个平台供应商，保留 {{skipped}} 个",
                      removed: app.removed,
                      skipped: app.skipped,
                    })}
                  </p>
                )}
                {app.warnings.map((warning, index) => (
                  <p key={index} className="mt-1 text-danger">
                    {warning}
                  </p>
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}
