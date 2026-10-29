#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);

function canonicalPath(target) {
  if (fs.existsSync(target)) return fs.realpathSync(target);
  // lstat also catches dangling symlinks, which must never be treated as new files.
  try {
    if (fs.lstatSync(target).isSymbolicLink())
      throw new Error(`Broken symlink: ${target}`);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const parent = path.dirname(target);
  if (parent === target) throw new Error(`Cannot resolve ${target}`);
  return path.join(canonicalPath(parent), path.basename(target));
}

function within(parent, child) {
  const relative = path.relative(parent, child);
  return (
    relative === "" ||
    (!relative.startsWith(`..${path.sep}`) &&
      relative !== ".." &&
      !path.isAbsolute(relative))
  );
}

export function resolveEnvironment(
  source = process.env,
  userHome = os.homedir(),
) {
  const requested =
    source.ZHISHU_CCS_DEV_HOME || path.join(userHome, ".zhishu-ccs-dev-home");
  if (!path.isAbsolute(requested))
    throw new Error("ZHISHU_CCS_DEV_HOME must be an absolute path");
  const home = canonicalPath(requested);
  const actualHome = canonicalPath(userHome);
  const protectedPaths = [
    ".codex",
    ".zhishu-ccs",
    ".cc-switch",
    ".claude",
    ".gemini",
    ".config",
  ];
  if (
    within(home, actualHome) ||
    protectedPaths.some((name) =>
      within(canonicalPath(path.join(userHome, name)), home),
    )
  ) {
    throw new Error(
      `Refusing to use the working environment as the dev home: ${home}`,
    );
  }
  const codexHome = path.join(home, ".codex");
  // Prevent existing symlinks from sending the initial files outside the dev home.
  for (const name of [
    ".zhishu-ccs",
    ".codex",
    ".codex/config.toml",
    ".codex/auth.json",
    ".codex/cc-switch-model-catalog.json",
  ]) {
    if (!within(home, canonicalPath(path.join(home, name)))) {
      throw new Error(`Dev path points outside the isolated home: ${name}`);
    }
  }
  const env = {
    ...source,
    ZHISHU_CCS_DEV_HOME: home,
    CODEX_HOME: codexHome,
  };
  delete env.CC_SWITCH_TEST_HOME;
  delete env.ZHISHU_CCS_DEV_PROXY_PORT;
  return {
    home,
    codexHome,
    env,
  };
}

export function prepareEnvironment(environment) {
  fs.mkdirSync(path.join(environment.home, ".zhishu-ccs"), {
    recursive: true,
    mode: 0o700,
  });
  fs.mkdirSync(environment.codexHome, { recursive: true, mode: 0o700 });
  // A new dev CLI uses file-based credentials rather than the working CLI's keychain.
  // Never copy credentials, pending writes, or absolute paths from the real environment.
  try {
    fs.writeFileSync(
      path.join(environment.codexHome, "config.toml"),
      'cli_auth_credentials_store = "file"\n',
      { flag: "wx", mode: 0o600 },
    );
  } catch (error) {
    if (error.code !== "EEXIST") throw error;
  }
}

export function buildCommand(args, platform = process.platform) {
  if (
    args.some(
      (arg) =>
        arg === "-c" || arg.startsWith("-c=") || arg.startsWith("--config"),
    )
  ) {
    throw new Error(
      "The isolated launcher owns the Tauri config; do not override --config",
    );
  }
  return platform === "darwin"
    ? {
        command: "sh",
        args: [path.join(projectRoot, "scripts/zhishu/dev-macos.sh"), ...args],
      }
    : { command: "pnpm", args: ["tauri", "dev", ...args] };
}

async function main(args) {
  if (args.includes("--help") || args.includes("-h")) {
    console.log(
      "Usage: node scripts/zhishu/dev-isolated.mjs [app] [--dry-run] [arguments...]\nEnvironment: ZHISHU_CCS_DEV_HOME (absolute path)",
    );
    return;
  }
  if (args[0] === "app") args.shift();
  const dryRun = args.includes("--dry-run");
  args = args.filter((arg) => arg !== "--dry-run");
  const environment = resolveEnvironment();
  const launch = buildCommand(args);
  console.log(
    `[DEV] App data: ${path.join(environment.home, ".zhishu-ccs")}\n[DEV] Codex: ${environment.codexHome}\n[DEV] Claude/Gemini and other CLI paths: production HOME`,
  );
  if (dryRun) {
    console.log(JSON.stringify({ action: "app", ...launch }, null, 2));
    return;
  }
  prepareEnvironment(environment);
  const child = spawn(launch.command, launch.args, {
    cwd: projectRoot,
    env: environment.env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  // Forward termination so an interrupted dev launcher does not leave a proxy behind.
  const onInterrupt = () => child.kill("SIGINT");
  const onTerminate = () => child.kill("SIGTERM");
  process.on("SIGINT", onInterrupt);
  process.on("SIGTERM", onTerminate);
  child.on("error", (error) => {
    console.error(`[DEV] Cannot start ${launch.command}: ${error.message}`);
    process.exitCode = 1;
  });
  child.on("exit", (code, signal) => {
    process.removeListener("SIGINT", onInterrupt);
    process.removeListener("SIGTERM", onTerminate);
    process.exitCode = code ?? (signal === "SIGINT" ? 130 : 1);
  });
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`[DEV] ${error.message}`);
    process.exitCode = 1;
  });
}
