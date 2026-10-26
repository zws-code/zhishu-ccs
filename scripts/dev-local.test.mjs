import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import {
  buildCommand,
  localEnvironment,
  projectRoot,
  realUserPaths,
} from "./dev-local.mjs";

test("local testing resolves the real user configuration paths", () => {
  const paths = realUserPaths("/Users/tester");
  assert.deepEqual(paths, {
    appData: "/Users/tester/.zhishu-ccs",
    codex: "/Users/tester/.codex",
    claude: "/Users/tester/.claude",
    gemini: "/Users/tester/.gemini",
  });
});

test("local testing removes isolation variables without changing other settings", () => {
  const env = localEnvironment({
    HOME: "/Users/tester",
    ZHISHU_CCS_DEV_HOME: "/tmp/dev",
    ZHISHU_CCS_DEV_PROXY_PORT: "35721",
    CC_SWITCH_TEST_HOME: "/tmp/test",
    CODEX_HOME: "/tmp/codex",
    KEEP_ME: "yes",
  });
  assert.deepEqual(env, { HOME: "/Users/tester", KEEP_ME: "yes" });
});

test("local testing uses the normal Tauri config and stable macOS launcher", () => {
  const env = {
    ZHISHU_CCS_DEV_HOME: "/tmp/dev",
    CC_SWITCH_TEST_HOME: "/tmp/test",
    CODEX_HOME: "/tmp/codex",
  };
  const mac = buildCommand(["--no-watch"], "darwin", env);
  assert.equal(mac.command, "sh");
  assert.deepEqual(mac.args, [
    path.join(projectRoot, "scripts/dev-macos.sh"),
    "--no-watch",
  ]);
  assert.deepEqual(mac.env, {});

  const linux = buildCommand([], "linux", env);
  assert.deepEqual(
    { command: linux.command, args: linux.args, env: linux.env },
    { command: "pnpm", args: ["tauri", "dev"], env: {} },
  );

  assert.throws(
    () => buildCommand(["--config", "test.json"], "linux", env),
    /do not override/,
  );
});
