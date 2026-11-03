import { invoke } from "@tauri-apps/api/core";
import type { AppId } from "@/lib/api/types";

export interface ProviderSyncReport {
  added: number;
  updated: number;
  removed: number;
  unchanged: number;
  changedFiles: string[];
  backupPath: string | null;
  warnings: string[];
}

/** One modified file inside an operation; the id addresses its record. */
export interface ConfigChangeFileSummary {
  id: string;
  path: string;
  beforeExists: boolean;
  afterExists: boolean;
  sensitive: boolean;
}

/** One restored file of a group restore. */
export interface ConfigRestoreFile {
  id: string;
  path: string;
  changed: boolean;
  backupPath: string | null;
}

export interface ConfigRestoreSummary {
  files: ConfigRestoreFile[];
  changed: boolean;
  pointerRestored: boolean;
}

/** Roster-style summary of an operation, e.g. aggregation membership changes. */
export interface ConfigOperationSummary {
  added: string[];
  removed: string[];
}

/** One operation: all files written together collapse into a single entry. */
export interface ConfigChangeGroup {
  id: string;
  app: AppId;
  path: string;
  changedAt: string;
  action: string;
  sensitive: boolean;
  files: ConfigChangeFileSummary[];
  sync?: ProviderSyncReport;
  summary?: ConfigOperationSummary;
}

export interface ConfigChangeDetail {
  id: string;
  app: AppId;
  path: string;
  changedAt: string;
  action: string;
  before: string | null;
  after: string | null;
  sensitive: boolean;
  sync?: ProviderSyncReport;
  summary?: ConfigOperationSummary;
}

export const configHistoryApi = {
  openSyncBackup: (id: string) =>
    invoke<void>("open_config_sync_backup", { id }),
  async list(app?: AppId): Promise<ConfigChangeGroup[]> {
    return await invoke("list_config_change_history", { app: app ?? null });
  },

  async get(
    id: string,
    revealSensitive = false,
  ): Promise<ConfigChangeDetail | null> {
    return await invoke("get_config_change_history", { id, revealSensitive });
  },

  async restore(ids: string[]): Promise<ConfigRestoreSummary> {
    return await invoke("restore_config_change_history", { ids });
  },

  async clear(): Promise<number> {
    return await invoke("clear_config_change_history");
  },
};
