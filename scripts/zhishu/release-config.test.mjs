import {
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  existsSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { prepareRelease } from "./prepare-release.mjs";
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  productionUrl,
  readReleaseConfig,
  validateReleaseConfig,
} from "./release-config.mjs";
const bytes = Buffer.concat([Buffer.from("Ed"), Buffer.alloc(40, 7)]);
const publicKey = Buffer.from(
  `untrusted comment: minisign public key: TEST\n${bytes.toString("base64")}\n`,
).toString("base64");
const config = {
  platformApiUrl: "https://api.zhishu.cn",
  platformWebUrl: "https://zhishu.cn",
  platformAllowInsecureHttp: false,
  updateUrl: "https://downloads.zhishu.cn/latest.json",
  updatePublicKey: publicKey,
  updateAllowInsecureHttp: false,
};
describe("Zhishu release configuration", () => {
  it("accepts own HTTPS services and a minisign public key", () => {
    assert.deepEqual(validateReleaseConfig(config), config);
  });
  it("refuses missing configuration before creating release files", () => {
    assert.throws(() => validateReleaseConfig({}), /configured/);
  });
  for (const url of [
    "http://127.0.0.1:3000",
    "https://localhost",
    "https://example.com",
    "https://api.zhishu.cn/path",
    "https://user:pass@zhishu.cn",
    "https://zhishu.cn?token=secret",
    "https://zhishu.cn/#secret",
  ]) {
    it(`rejects nonproduction platform origin ${url}`, () => {
      assert.throws(() => productionUrl(url, "platform", { originOnly: true }));
    });
  }
  it("allows explicit HTTP intranet endpoints without allowing loopback", () => {
    const intranet = {
      ...config,
      platformApiUrl: "http://192.168.10.20:3000",
      platformWebUrl: "http://192.168.10.20:3081",
      platformAllowInsecureHttp: true,
    };
    assert.deepEqual(validateReleaseConfig(intranet), intranet);
    for (const url of ["http://127.0.0.1:3000", "http://[::1]:3000"]) {
      assert.throws(
        () =>
          productionUrl(url, "platform", {
            originOnly: true,
            allowInsecureHttp: true,
          }),
        /loopback/,
      );
    }
  });
  it("rejects original client update sources", () => {
    for (const updateUrl of [
      "https://dl.ccswitch.io/latest.json",
      "https://github.com/farion1231/cc-switch/releases/latest/download/latest.json",
    ])
      assert.throws(
        () => validateReleaseConfig({ ...config, updateUrl }),
        /own release/,
      );
  });
  it("rejects malformed public keys", () => {
    assert.throws(
      () => validateReleaseConfig({ ...config, updatePublicKey: "not-a-key" }),
      /public key/,
    );
  });
  it("requires both an updater URL and a signing public key", () => {
    assert.throws(
      () => validateReleaseConfig({ ...config, updateUrl: "" }),
      /updateUrl/,
    );
    assert.throws(
      () => validateReleaseConfig({ ...config, updatePublicKey: "" }),
      /public key/,
    );
  });
});

it("applies strict independent HTTP environment overrides to the local release config", () => {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-env-test-"));
  const root = pathToFileURL(dir + "/");
  try {
    writeFileSync(
      join(dir, "release.config.local.json"),
      JSON.stringify(config),
    );
    for (const name of [
      "ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP",
      "ZHISHU_UPDATE_ALLOW_INSECURE_HTTP",
    ]) {
      const field =
        name === "ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP"
          ? "platformAllowInsecureHttp"
          : "updateAllowInsecureHttp";
      assert.equal(readReleaseConfig(root, { [name]: "true" })[field], true);
      assert.equal(readReleaseConfig(root, { [name]: "false" })[field], false);
      for (const value of ["", "1", "TRUE"])
        assert.throws(
          () => readReleaseConfig(root, { [name]: value }),
          new RegExp(name),
        );
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

it("writes matching public renderer and native updater configuration only after validation", () => {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-release-test-"));
  const root = pathToFileURL(dir + "/");
  try {
    mkdirSync(join(dir, "src"));
    mkdirSync(join(dir, "src-tauri"));
    assert.throws(() => prepareRelease(root, {}));
    assert.equal(existsSync(join(dir, "src/.env.production.local")), false);
    writeFileSync(
      join(dir, "release.config.local.json"),
      JSON.stringify(config),
    );
    prepareRelease(root, {});
    const env = readFileSync(join(dir, "src/.env.production.local"), "utf8");
    assert.match(env, /VITE_PLATFORM_API_URL=https:\/\/api.zhishu.cn/);
    assert.match(env, /VITE_ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP=false/);
    assert.doesNotMatch(env, /VITE_ZHISHU_PLATFORM_SERVER_LOCKED/);
    assert.match(env, /VITE_ZHISHU_UPDATES_ENABLED=true/);
    const native = JSON.parse(
      readFileSync(
        join(dir, "src-tauri/tauri.release.local.conf.json"),
        "utf8",
      ),
    );
    assert.equal(native.bundle.createUpdaterArtifacts, true);
    assert.deepEqual(native.plugins.updater, {
      endpoints: [config.updateUrl],
      pubkey: publicKey,
    });
    writeFileSync(
      join(dir, "release.config.local.json"),
      JSON.stringify({
        ...config,
        updateUrl: "http://192.168.20.10:8080/client-downloads/latest.json",
        updateAllowInsecureHttp: true,
      }),
    );
    prepareRelease(root, {});
    const httpConfig = JSON.parse(
      readFileSync(
        join(dir, "src-tauri/tauri.release.local.conf.json"),
        "utf8",
      ),
    );
    assert.equal(
      httpConfig.plugins.updater.dangerousInsecureTransportProtocol,
      true,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

it("generates a website manifest from real installer files and rejects upstream URLs", () => {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-manifest-test-"));
  const tool = fileURLToPath(
    new URL("../generate-download-manifest.mjs", import.meta.url),
  );
  try {
    const name = "Zhishu-Ccs-1.0.0-Windows-arm64.msi";
    writeFileSync(join(dir, name), "fixture installer");
    const output = join(dir, "client-release.json");
    const result = spawnSync(
      process.execPath,
      [
        tool,
        dir,
        "v1.0.0",
        "https://downloads.zhishu.cn",
        output,
        "2026-10-23T00:00:00Z",
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const manifest = JSON.parse(readFileSync(output, "utf8"));
    assert.equal(manifest.version, "1.0.0");
    assert.equal(manifest.files[0].arch, "arm64");
    assert.equal(
      manifest.files[0].url,
      `https://downloads.zhishu.cn/v1.0.0/${name}`,
    );
    assert.equal(
      manifest.files[0].sha256,
      createHash("sha256").update("fixture installer").digest("hex"),
    );
    const blocked = spawnSync(
      process.execPath,
      [
        tool,
        dir,
        "v1.0.0",
        "https://dl.ccswitch.io",
        join(dir, "blocked.json"),
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(blocked.status, 0);
    assert.equal(existsSync(join(dir, "blocked.json")), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

it("build entry enables signed updates and forwards Tauri arguments", () => {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-build-entry-"));
  try {
    for (const name of ["scripts/zhishu", "src", "src-tauri"])
      mkdirSync(join(dir, name), { recursive: true });
    for (const name of [
      "build-release.mjs",
      "prepare-release.mjs",
      "release-config.mjs",
    ])
      copyFileSync(new URL(name, import.meta.url), join(dir, "scripts/zhishu", name));
    writeFileSync(
      join(dir, "release.config.local.json"),
      JSON.stringify(config),
    );
    const manager = join(dir, "manager.cjs");
    const capture = join(dir, "invocation.json");
    writeFileSync(
      manager,
      `require("node:fs").writeFileSync(process.env.ZHISHU_TEST_CAPTURE, JSON.stringify({ args: process.argv.slice(2) }));`,
    );
    const result = spawnSync(
      process.execPath,
      [
        join(dir, "scripts/zhishu/build-release.mjs"),
        "--",
        "--target",
        "fixture-target",
        "--ci",
      ],
      {
        cwd: dir,
        encoding: "utf8",
        env: {
          ...process.env,
          npm_execpath: manager,
          ZHISHU_TEST_CAPTURE: capture,
          VITE_PLATFORM_API_URL: config.platformApiUrl,
          VITE_PLATFORM_WEB_URL: config.platformWebUrl,
          ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP: "false",
          ZHISHU_UPDATE_URL: config.updateUrl,
          ZHISHU_UPDATE_PUBLIC_KEY: config.updatePublicKey,
          ZHISHU_UPDATE_ALLOW_INSECURE_HTTP: "false",
        },
      },
    );
    assert.equal(result.status, 0, result.stderr);
    const invocation = JSON.parse(readFileSync(capture));
    assert.deepEqual(invocation.args, [
      "tauri",
      "build",
      "--config",
      "src-tauri/tauri.release.local.conf.json",
      "--target",
      "fixture-target",
      "--ci",
    ]);
    const renderer = readFileSync(
      join(dir, "src/.env.production.local"),
      "utf8",
    );
    assert.ok(!renderer.includes("VITE_ZHISHU_PLATFORM_SERVER_LOCKED"));
    assert.ok(
      renderer.includes("VITE_ZHISHU_UPDATES_ENABLED=true"),
    );
    const native = JSON.parse(
      readFileSync(join(dir, "src-tauri/tauri.release.local.conf.json")),
    );
    assert.equal(native.bundle.createUpdaterArtifacts, true);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
