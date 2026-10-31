import { Loader2, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import type { usePlatformProviderDeletion } from "../usePlatformProviderDeletion";

/** The parent keeps the mutation alive while the dropdown is closed. */
export function PlatformProviderDeletionMenuItem({
  deletion,
}: {
  deletion: ReturnType<typeof usePlatformProviderDeletion>;
}) {
  const { t } = useTranslation();
  if (!deletion.visible) return null;
  return (
    <>
      <DropdownMenuSeparator />
      <DropdownMenuItem
        disabled={deletion.disabled}
        onSelect={deletion.remove}
        className="text-destructive focus:text-destructive"
      >
        {deletion.pending ? (
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        ) : (
          <Trash2 className="mr-2 h-4 w-4" />
        )}
        {t(
          deletion.pending
            ? "platformCatalog.deletingProviders"
            : "platformCatalog.deleteProviders",
        )}
      </DropdownMenuItem>
    </>
  );
}
