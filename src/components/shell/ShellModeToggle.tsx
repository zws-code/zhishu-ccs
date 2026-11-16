import { useTranslation } from "react-i18next";
import { LayoutGrid, PanelsTopLeft } from "lucide-react";
import { Button } from "@/components/ui/button";

/**
 * 右下角的外壳模式切换：高级（完整侧栏）/ 简易（应用开关）互切。
 * 纯布局偏好（localStorage），不触发任何配置读写；按钮显示的是要切去的模式。
 */
export function ShellModeToggle({
  simple,
  onToggle,
}: {
  simple: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Button
      variant="neutral"
      size="compact"
      className="absolute bottom-4 end-4 z-40 shadow-v7-sm"
      onClick={onToggle}
      aria-pressed={simple}
      aria-label={
        simple
          ? t("simpleMode.toAdvanced", "高级模式")
          : t("simpleMode.toSimple", "简易模式")
      }
    >
      {simple ? (
        <PanelsTopLeft className="h-3.5 w-3.5" aria-hidden="true" />
      ) : (
        <LayoutGrid className="h-3.5 w-3.5" aria-hidden="true" />
      )}
      {simple
        ? t("simpleMode.toAdvanced", "高级模式")
        : t("simpleMode.toSimple", "简易模式")}
    </Button>
  );
}
