import type { AppId } from "@/lib/api/types";

/**
 * 平台供应商同步的能力白名单，与后端 zhishu/integrations/ccswitch/platform_models.rs::AGENTS 对齐。
 * 此处只决定平台同步入口，不代表该工具同时支持 MCP、Skills、路由或聚合模式。
 */
export const PLATFORM_SYNC_APPS: ReadonlySet<AppId> = new Set([
  "claude",
  "claude-desktop",
  "codex",
  "gemini",
  "grokbuild",
  "opencode",
  "openclaw",
  "hermes",
  "pi",
  "workbuddy",
  "mcode",
  "kimi",
  "dsh",
]);
