import { invoke } from "@tauri-apps/api/core";
import type { PlatformDeleteSummary } from "./platformModels";

export interface PlatformAccount {
  baseUrl: string;
  user: {
    id: number;
    email: string;
    username: string;
    role: string;
    status?: string;
    balance: number;
  };
}
export interface PlatformCleanupStep {
  completed: boolean;
  error: { code: string; message: string } | null;
}
export interface PlatformCleanupReport {
  baseUrl: string;
  userId: number;
  email: string;
  providers: PlatformDeleteSummary | null;
  providerError: { code: string; message: string } | null;
  catalog: PlatformCleanupStep;
  session: PlatformCleanupStep;
  serverLogout: PlatformCleanupStep;
}

export interface PlatformClientConfig {
  schema_version: number;
  site_name: string;
  api_base_url: string;
  login_methods: string[];
  subscriptions_url: string | null;
  // 旧服务端缺少授权字段时显示不可用提示，不回退到密码登录。
  oauth_authorize_url?: string | null;
  oauth_client_id?: string;
  oauth_scopes?: string[];
}
export interface PlatformLoginResult {
  status: "authenticated";
  account: PlatformAccount;
  persistenceWarning?: string;
}

/** OAuth2 登录 IPC：授权码与 access/refresh token 均留在 Rust。 */
export const platformAuthApi = {
  config: (baseUrl: string) =>
    invoke<PlatformClientConfig>("platform_get_client_config", { baseUrl }),
  account: (active = true) =>
    invoke<PlatformAccount | null>("platform_get_account", { active }),
  cancelLogin: () => invoke<void>("platform_cancel_login"),
  // 打开系统浏览器完成平台授权；回调、令牌交换和持久化都在 Rust 侧完成。
  loginOAuth: (baseUrl: string) =>
    invoke<PlatformLoginResult>("platform_login_oauth", { baseUrl }),
  logout: () => invoke<void>("platform_logout"),
  logoutAndClear: (baseUrl: string, userId: number) =>
    invoke<PlatformCleanupReport>("platform_logout_and_clear_account", {
      baseUrl,
      userId,
    }),
};

export function platformAuthError(error: unknown): string {
  if (
    error &&
    typeof error === "object" &&
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  // 只展示后端定义的平台错误，避免把可能含请求参数和凭据的任意 IPC 异常直接显示给用户。
  return "无法完成平台请求，请稍后重试";
}
