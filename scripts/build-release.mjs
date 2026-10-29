import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { prepareRelease } from "./prepare-release.mjs";

const requestedArgs = process.argv.slice(2);
if (requestedArgs.some((arg) => arg === "-c" || arg.startsWith("--config")))
  throw new Error("Release configuration cannot be overridden");
const args = requestedArgs.filter((arg) => arg !== "--");

const root = fileURLToPath(new URL("../", import.meta.url));
prepareRelease(new URL("../", import.meta.url));
const tauriArgs = [
  "tauri",
  "build",
  "--config",
  "src-tauri/tauri.release.local.conf.json",
  ...args,
];
const packageManager = process.env.npm_execpath;
if (!packageManager) throw new Error("Run this script with pnpm build");
const packageManagerIsScript = /\.(?:cjs|mjs|js)$/i.test(packageManager);
const built = spawnSync(
  packageManagerIsScript ? process.execPath : packageManager,
  packageManagerIsScript ? [packageManager, ...tauriArgs] : tauriArgs,
  { cwd: root, env: process.env, stdio: "inherit" },
);
if (built.error) throw built.error;
process.exit(built.status ?? 1);
