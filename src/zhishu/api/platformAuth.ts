import { invoke } from "@tauri-apps/api/core";

export interface PlatformAccount {
  baseUrl: string;
  user: {
    id: number;
    email: string;
    username: string;
    role: string;
    balance: number;
  };
}
export interface SavedPlatformLogin {
  id: string;
  baseUrl: string;
  email: string;
  rememberPassword: boolean;
}

export type PlatformLoginResult =
  | {
      status: "authenticated";
      account: PlatformAccount;
      persistenceWarning?: string;
    }
  | { status: "requires_2fa" };

/** 平台登录 IPC：access/refresh token 留在 Rust；保存密码只在用户选择历史账号时显式读取。 */
export const platformAuthApi = {
  account: (active = true) =>
    invoke<PlatformAccount | null>("platform_get_account", { active }),
  history: () => invoke<SavedPlatformLogin[]>("platform_login_history"),
  savedPassword: (id: string) =>
    invoke<string | null>("platform_saved_password", { id }),
  forgetPassword: (id: string) =>
    invoke<void>("platform_forget_password", { id }),
  removeLogin: (id: string) => invoke<void>("platform_remove_login", { id }),
  cancelLogin: () => invoke<void>("platform_cancel_login"),
  login: (
    baseUrl: string,
    email: string,
    password: string,
    rememberPassword = false,
  ) =>
    invoke<PlatformLoginResult>("platform_login", {
      baseUrl,
      email,
      password,
      rememberPassword,
    }),
  login2fa: (code: string) =>
    invoke<PlatformLoginResult>("platform_login_2fa", { code }),
  logout: () => invoke<void>("platform_logout"),
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
