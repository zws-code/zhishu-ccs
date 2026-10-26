import path from "node:path";
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { codeInspectorPlugin } from "code-inspector-plugin";
import { productionUrl } from "./scripts/release-config.mjs";

export default defineConfig(({ command, mode }) => {
  if (command === "build" && mode === "production") {
    const env = loadEnv(mode, path.resolve(__dirname, "src"), "VITE_");
    const allowInsecureHttp =
      env.VITE_ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP === "true";
    if (
      env.VITE_ZHISHU_UPDATES_ENABLED !== "true" &&
      env.VITE_ZHISHU_UPDATES_ENABLED !== "false"
    ) {
      throw new Error(
        "Configure VITE_ZHISHU_UPDATES_ENABLED as true or false before a production build (see docs/zhishu-release.md)",
      );
    }
    for (const key of ["VITE_PLATFORM_API_URL", "VITE_PLATFORM_WEB_URL"]) {
      try {
        productionUrl(env[key], key, {
          originOnly: true,
          allowInsecureHttp,
        });
      } catch (error) {
        throw new Error(
          `Configure ${key} before a production build: ${error.message}`,
        );
      }
    }
  }
  return {
    root: "src",
    plugins: [
      command === "serve" &&
        codeInspectorPlugin({
          bundler: "vite",
        }),
      react(),
    ].filter(Boolean),
    base: "./",
    build: {
      outDir: "../dist",
      emptyOutDir: true,
      rollupOptions: {
        // 托盘面板是单独的入口，不加载主界面
        input: {
          main: path.resolve(__dirname, "src/index.html"),
          "tray-panel": path.resolve(__dirname, "src/tray-panel.html"),
        },
      },
    },
    server: {
      port: 1420,
      strictPort: true,
    },
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    clearScreen: false,
    envPrefix: ["VITE_", "TAURI_"],
  };
});
