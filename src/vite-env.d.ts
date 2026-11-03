/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_PLATFORM_API_URL?: string;
  readonly VITE_PLATFORM_WEB_URL?: string;
  readonly VITE_ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP?: string;
  readonly VITE_ZHISHU_UPDATES_ENABLED?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

export {};
