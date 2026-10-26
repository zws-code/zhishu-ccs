import { useEffect, useRef, useState } from "react";
import { Copy, Eye, EyeOff, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { platformModelsApi } from "@/lib/api/platformModels";
import { platformAuthError } from "@/lib/api/platformAuth";
import { copyText } from "@/lib/clipboard";
import { toast } from "@/lib/toast";

/** 仅在打开的详情抽屉内保存临时明文，关闭即卸载；查看和复制不把 Key 写入查询缓存。 */
export function PlatformCatalogKey({
  baseUrl,
  userId,
  groupId,
  available = true,
}: {
  baseUrl: string;
  userId: number;
  groupId: number;
  available?: boolean;
}) {
  const { t } = useTranslation();
  const [secret, setSecret] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const read = async (copy: boolean) => {
    if (busy) return;
    setBusy(true);
    try {
      const key =
        secret ??
        (await platformModelsApi.catalogKey(baseUrl, userId, groupId));
      if (!mounted.current) return;
      if (!key) {
        toast.warning(
          t(
            "platformCatalog.noKey",
            "当前分组暂无可用的 API Key，请重新同步基础数据",
          ),
        );
      } else if (copy) {
        try {
          await copyText(key);
          if (mounted.current)
            toast.success(t("platformCatalog.keyCopied", "API Key 已复制"));
        } catch {
          if (mounted.current)
            toast.error(
              t("platformCatalog.copyFailed", "无法复制 API Key，请重试"),
            );
        }
      } else {
        setSecret(key);
      }
    } catch (error) {
      if (mounted.current) toast.error(platformAuthError(error));
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  if (!available)
    return (
      <p className="text-caption text-fg-3">
        {t(
          "platformCatalog.noKey",
          "当前分组暂无可用的 API Key，请重新同步基础数据",
        )}
      </p>
    );

  return (
    <div className="flex items-start gap-2 rounded-control border border-border bg-subtle px-3 py-2">
      <code
        aria-label="API Key"
        className="min-w-0 flex-1 select-text break-all py-1 text-caption"
      >
        {secret ?? "••••••••••••••••••••••••"}
      </code>
      <Button
        variant="ghost"
        size="icon-compact"
        disabled={busy}
        aria-label={
          secret
            ? t("platformCatalog.hideKey", "隐藏 API Key")
            : t("platformCatalog.showKey", "显示 API Key")
        }
        onClick={() => (secret ? setSecret(null) : void read(false))}
      >
        {busy ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : secret ? (
          <EyeOff className="h-4 w-4" />
        ) : (
          <Eye className="h-4 w-4" />
        )}
      </Button>
      <Button
        variant="ghost"
        size="icon-compact"
        disabled={busy}
        aria-label={t("platformCatalog.copyKey", "复制 API Key")}
        onClick={() => void read(true)}
      >
        <Copy className="h-4 w-4" />
      </Button>
    </div>
  );
}
