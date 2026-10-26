import { useTranslation } from "react-i18next";
import { ChevronsLeft, ChevronsRight } from "lucide-react";
import { HoverTip } from "@/components/ui/hover-tip";
import { DRAG_REGION_ATTR, DRAG_REGION_STYLE, isMac } from "@/lib/platform";
import { cn } from "@/lib/utils";
import ccswitchLogo from "@/assets/icons/logo.png";

const NO_DRAG = { WebkitAppRegion: "no-drag" } as React.CSSProperties;

export function SidebarTopBar({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const tipSide = collapsed ? "right" : "left";
  const label = collapsed ? t("nav.expandSidebar") : t("nav.collapseSidebar");
  const Icon = collapsed ? ChevronsRight : ChevronsLeft;
  const toggleButton = (
    <HoverTip content={label} side={tipSide} disableHoverableContent>
      <button
        type="button"
        onClick={onToggle}
        aria-label={label}
        style={NO_DRAG}
        className="flex h-7 w-7 items-center justify-center rounded-control text-fg-2 transition-[background-color,color,scale] hover:bg-subtle hover:text-fg-1 active:scale-[0.96]"
      >
        <Icon className="h-4 w-4" strokeWidth={1.5} />
      </button>
    </HoverTip>
  );

  // macOS 展开：顶条只放红绿灯；下面一行贴近红绿灯放 logo + 名字，收起箭头放在这一行最右边
  if (!collapsed && isMac()) {
    return (
      <>
        <div
          className="h-11 shrink-0"
          {...DRAG_REGION_ATTR}
          style={DRAG_REGION_STYLE as React.CSSProperties}
        />
        {/* 品牌区与导航之间不画分隔线：靠侧栏底色和留白分开，免得和右侧页头下边框近似平行却差 11px */}
        <div
          className="-mt-[18px] mb-2 flex h-9 shrink-0 items-center gap-2 pe-2 ps-4"
          {...DRAG_REGION_ATTR}
          style={DRAG_REGION_STYLE as React.CSSProperties}
        >
          <img
            src={ccswitchLogo}
            alt=""
            draggable={false}
            className="pointer-events-none h-8 w-8 shrink-0 object-contain dark:invert"
          />
          <span className="pointer-events-none me-auto min-w-0 truncate text-strong font-semibold text-fg-1">
            CC Switch
          </span>
          {toggleButton}
        </div>
      </>
    );
  }

  // macOS 的红绿灯占着顶条左边约 70px：图标轨只有 72 宽，按钮挪到顶条下面一行
  if (collapsed && isMac()) {
    return (
      <>
        <div
          className="h-11 shrink-0"
          {...DRAG_REGION_ATTR}
          style={DRAG_REGION_STYLE as React.CSSProperties}
        />
        {/* 往上贴近红绿灯，和展开时品牌行同高（中心 y≈44） */}
        <div className="-mt-4 flex h-8 shrink-0 items-center justify-center">
          {toggleButton}
        </div>
      </>
    );
  }

  return (
    <div
      className={cn(
        "flex h-11 shrink-0 items-center gap-0.5 px-2",
        collapsed ? "justify-center" : "justify-end",
      )}
      {...DRAG_REGION_ATTR}
      style={DRAG_REGION_STYLE as React.CSSProperties}
    >
      {!collapsed && !isMac() && (
        // 左上角品牌（Windows / Linux）：图标 + 名字。macOS 左上角留给红绿灯，不放。
        // 图片不接收指针事件，拖它也是在拖窗口
        <span className="pointer-events-none me-auto flex min-w-0 items-center gap-2 ps-1.5">
          <img
            src={ccswitchLogo}
            alt=""
            draggable={false}
            className="h-8 w-8 shrink-0 object-contain dark:invert"
          />
          <span className="truncate text-strong font-semibold text-fg-1">
            CC Switch
          </span>
        </span>
      )}
      {toggleButton}
    </div>
  );
}
