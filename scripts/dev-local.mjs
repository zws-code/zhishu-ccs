#!/usr/bin/env node
import { spawn } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

const ISOLATION_ENV_KEYS = [
  "ZHISHU_CCS_DEV_HOME",
  "ZHISHU_CCS_DEV_PROXY_PORT",
  "CC_SWITCH_TEST_HOME",
  "CODEX_HOME",
];

export function realUserPaths(userHome = os.homedir()) {
  return {
    appData: path.join(userHome, ".zhishu-ccs"),
    codex: path.join(userHome, ".codex"),
    claude: path.join(userHome, ".claude"),
    gemini: path.join(userHome, ".gemini"),
  };
}

export function localEnvironment(source = process.env) {
  const env = { ...source };
  for (const key of ISOLATION_ENV_KEYS) {
    delete env[key];
  }
  return env;
}

export function buildCommand(
  args,
  platform = process.platform,
  sourceEnv = process.env,
) {
  if (
    args.some(
      (arg) =>
        arg === "-c" || arg.startsWith("-c=") || arg.startsWith("--config"),
    )
  ) {
    throw new Error(
      "Local testing uses src-tauri/tauri.conf.json; do not override --config",
    );
  }

  const env = localEnvironment(sourceEnv);
  return platform === "darwin"
    ? {
        command: "sh",
        args: [path.join(projectRoot, "scripts/dev-macos.sh"), ...args],
        env,
      }
    : { command: "pnpm", args: ["tauri", "dev", ...args], env };
}

function printPlan(paths) {
  console.log(
    [
      "[LOCAL TEST] Using real user configuration:",
      `[LOCAL TEST] App data: ${paths.appData}`,
      `[LOCAL TEST] Codex:    ${paths.codex}`,
      `[LOCAL TEST] Claude:   ${paths.claude}`,
      `[LOCAL TEST] Gemini:   ${paths.gemini}`,
      "[LOCAL TEST] This command can modify live CLI settings and credentials.",
      "[LOCAL TEST] Close the installed app before starting to avoid conflicts.",
    ].join("\n"),
  );
}

async function main(args) {
  if (args.includes("--help") || args.includes("-h")) {
    console.log(
      "Usage: node scripts/dev-local.mjs [--dry-run] [tauri dev arguments...]\n" +
        "Starts the development app against the real user configuration.",
    );
    return;
  }

  const dryRun = args.includes("--dry-run");
  args = args.filter((arg) => arg !== "--dry-run");
  const paths = realUserPaths();
  const launch = buildCommand(args);
  printPlan(paths);
  if (dryRun) {
    console.log(
      JSON.stringify(
        { ...launch, env: "[isolation variables removed]" },
        null,
        2,
      ),
    );
    return;
  }

  const child = spawn(launch.command, launch.args, {
    cwd: projectRoot,
    env: launch.env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  const onInterrupt = () => child.kill("SIGINT");
  const onTerminate = () => child.kill("SIGTERM");
  process.on("SIGINT", onInterrupt);
  process.on("SIGTERM", onTerminate);
  child.on("error", (error) => {
    console.error(
      `[LOCAL TEST] Cannot start ${launch.command}: ${error.message}`,
    );
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
    console.error(`[LOCAL TEST] ${error.message}`);
    process.exitCode = 1;
  });
}
