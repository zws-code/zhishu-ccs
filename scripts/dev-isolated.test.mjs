import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  buildCommand,
  prepareEnvironment,
  projectRoot,
  resolveEnvironment,
} from "./dev-isolated.mjs";

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "zhishu-dev-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const userHome = path.join(root, "user");
  fs.mkdirSync(userHome);
  return { root, userHome };
}

test("development isolation preserves real Codex files and existing dev config", (t) => {
  const { userHome } = fixture(t);
  const realCodex = path.join(userHome, ".codex");
  fs.mkdirSync(realCodex);
  const realConfig = path.join(realCodex, "config.toml");
  fs.writeFileSync(
    realConfig,
    'openai_base_url = "http://127.0.0.1:15721/v1"\n',
  );
  fs.writeFileSync(path.join(realCodex, "auth.json"), "working-credentials");
  const original = fs.readFileSync(realConfig);
  const source = {
    CODEX_HOME: realCodex,
    CC_SWITCH_TEST_HOME: userHome,
    ZHISHU_CCS_DEV_PROXY_PORT: "35721",
  };
  const environment = resolveEnvironment(source, userHome);
  prepareEnvironment(environment);
  assert.deepEqual(fs.readFileSync(realConfig), original);
  assert.equal(source.CODEX_HOME, realCodex);
  assert.equal(
    environment.home,
    path.join(fs.realpathSync(userHome), ".zhishu-ccs-dev-home"),
  );
  assert.equal(environment.env.CODEX_HOME, environment.codexHome);
  assert.equal(environment.env.ZHISHU_CCS_DEV_HOME, environment.home);
  assert.equal(environment.env.CC_SWITCH_TEST_HOME, undefined);
  assert.equal(environment.env.ZHISHU_CCS_DEV_PROXY_PORT, undefined);
  assert.equal(
    fs.existsSync(path.join(environment.codexHome, "auth.json")),
    false,
  );
  const config = path.join(environment.codexHome, "config.toml");
  assert.equal(
    fs.readFileSync(config, "utf8"),
    'cli_auth_credentials_store = "file"\n',
  );
  fs.writeFileSync(config, 'model = "test-model"\n');
  prepareEnvironment(environment);
  assert.equal(fs.readFileSync(config, "utf8"), 'model = "test-model"\n');
});

test("rejects real homes, protected configuration paths, and symlink escapes", (t) => {
  const { root, userHome } = fixture(t);
  for (const home of [
    userHome,
    root,
    path.join(userHome, ".codex"),
    path.join(userHome, ".zhishu-ccs", "test"),
    "relative",
  ]) {
    assert.throws(() =>
      resolveEnvironment({ ZHISHU_CCS_DEV_HOME: home }, userHome),
    );
  }
  const devHome = path.join(userHome, ".zhishu-ccs-dev-home");
  fs.mkdirSync(devHome);
  const realCodex = path.join(userHome, ".codex");
  fs.mkdirSync(realCodex);
  fs.symlinkSync(realCodex, path.join(devHome, ".codex"), "dir");
  assert.throws(() => resolveEnvironment({}, userHome), /outside/);
  fs.unlinkSync(path.join(devHome, ".codex"));
  fs.mkdirSync(path.join(devHome, ".codex"));
  fs.symlinkSync(
    path.join(root, "missing"),
    path.join(devHome, ".codex", "config.toml"),
  );
  assert.throws(() => resolveEnvironment({}, userHome), /Broken symlink/);
});

test("app launch plans use the production Tauri config", () => {
  const launch = buildCommand(["--no-watch"], "darwin");
  assert.equal(launch.command, "sh");
  assert.deepEqual(launch.args, [
    path.join(projectRoot, "scripts/dev-macos.sh"),
    "--no-watch",
  ]);
  assert.throws(() => buildCommand(["--config", "real.json"]), /owns/);
  assert.deepEqual(buildCommand([], "linux"), {
    command: "pnpm",
    args: ["tauri", "dev"],
  });
});
