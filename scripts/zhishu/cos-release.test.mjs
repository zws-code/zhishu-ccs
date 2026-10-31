import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  artifactNames,
  collectReceiptUpdaterPlatforms,
  hashFile,
  legacyTargets,
  targets,
} from "./release-artifacts.mjs";
import { checkVersion, cosConfig, prepareCosPayload } from "./cos-release.mjs";

function fixture(fn) {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-cos-test-"));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
function identity() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const id = Buffer.from("0011223344556677", "hex");
  const raw = publicKey.export({ format: "der", type: "spki" }).subarray(-32);
  const key = Buffer.from(
    `untrusted comment: minisign public key: TEST\n${Buffer.concat([Buffer.from("Ed"), id, raw]).toString("base64")}\n`,
  ).toString("base64");
  const signature = (data) => {
    const bytes = sign(
      null,
      createHash("blake2b512").update(data).digest(),
      privateKey,
    );
    const comment = "timestamp:123456\tfile:fixture";
    const global = sign(
      null,
      Buffer.concat([bytes, Buffer.from(comment)]),
      privateKey,
    );
    return Buffer.from(
      `untrusted comment: signature\n${Buffer.concat([Buffer.from("ED"), id, bytes]).toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`,
    ).toString("base64");
  };
  return { key, signature };
}
const environment = (key) => ({
  COS_BUCKET: "zhishu-downloads-1234567890",
  COS_REGION: "ap-guangzhou",
  COS_PUBLIC_BASE_URL: "https://download.zhishu.cn/ccs/stable",
  ZHISHU_UPDATE_URL: "https://download.zhishu.cn/ccs/stable/latest.json",
  ZHISHU_UPDATE_PUBLIC_KEY: key,
  VITE_PLATFORM_API_URL: "https://api.zhishu.cn",
  VITE_PLATFORM_WEB_URL: "https://www.zhishu.cn",
  ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP: "false",
  ZHISHU_UPDATE_ALLOW_INSECURE_HTTP: "false",
});
function payload(dir, mutate = () => {}, definitions = targets) {
  const key = identity();
  const assets = join(dir, "assets");
  mkdirSync(assets);
  for (const id of Object.keys(definitions)) {
    for (const [ext, name] of Object.entries(
      artifactNames(id, "1.0.0", definitions),
    )) {
      const data = Buffer.from(`fixture ${name}`);
      writeFileSync(join(assets, name), data);
      if (!["dmg", "zip"].includes(ext))
        writeFileSync(join(assets, name + ".sig"), key.signature(data));
    }
  }
  writeFileSync(
    join(assets, "latest.json"),
    JSON.stringify({
      version: "1.0.0",
      notes: "First release",
      pub_date: "2026-10-29T03:00:00Z",
    }),
  );
  mutate(assets);
  const out = join(dir, "out");
  return { key, assets, out, env: environment(key.key) };
}

test("COS configuration requires a dedicated HTTPS prefix and matching update endpoint", () => {
  const env = environment(identity().key);
  assert.equal(cosConfig(env).prefix, "ccs/stable");
  for (const change of [
    { COS_PREFIX: "../stable" },
    { COS_PREFIX: "/ccs/stable" },
    { COS_BUCKET: "no-appid" },
    { COS_REGION: "" },
    { COS_PUBLIC_BASE_URL: "http://download.zhishu.cn/ccs/stable" },
    { COS_PUBLIC_BASE_URL: "https://download.zhishu.cn/another" },
    { ZHISHU_UPDATE_URL: "https://download.zhishu.cn/old/latest.json" },
    { COS_PUBLIC_BASE_URL: "https://dl.ccswitch.io/ccs/stable" },
  ])
    assert.throws(() => cosConfig({ ...env, ...change }));
});

test("tag must match all three committed versions and an existing summary", () =>
  fixture((dir) => {
    mkdirSync(join(dir, "src-tauri"));
    mkdirSync(join(dir, "src/whats-new"), { recursive: true });
    writeFileSync(join(dir, "package.json"), '{"version":"1.0.0"}');
    writeFileSync(
      join(dir, "src-tauri/tauri.conf.json"),
      '{"version":"1.0.0"}',
    );
    writeFileSync(
      join(dir, "src-tauri/Cargo.toml"),
      '[package]\nversion = "1.0.0"\n',
    );
    assert.throws(() => checkVersion(dir, "v1.0.0"), /更新说明/);
    writeFileSync(
      join(dir, "src/whats-new/1.0.0.json"),
      '{"version":"1.0.0","items":[]}',
    );
    assert.equal(checkVersion(dir, "v1.0.0"), "1.0.0");
    assert.throws(() => checkVersion(dir, "v1.0.1"), /不一致/);
    assert.throws(() => checkVersion(dir, "v1.0.0-beta"));
  }));

test("all platforms get verified signatures and CDN manifests with accurate sizes and hashes", () =>
  fixture((dir) => {
    const { assets, out, env } = payload(dir);
    const receipt = prepareCosPayload(assets, "v1.0.0", out, env);
    assert.equal(receipt.targets.length, 3);
    const latest = JSON.parse(readFileSync(join(out, "latest.json")));
    const website = JSON.parse(readFileSync(join(out, "client-release.json")));
    assert.equal(Object.keys(latest.platforms).length, 4);
    assert.equal(website.files.length, 4);
    assert.equal(receipt.packaging, "nsis-v1");
    assert.deepEqual(Object.keys(latest.platforms).sort(), [
      "darwin-aarch64",
      "darwin-x86_64",
      "windows-aarch64",
      "windows-x86_64",
    ]);
    assert.deepEqual(
      website.files
        .filter((file) => file.platform === "windows")
        .map(({ arch, kind }) => ({ arch, kind })),
      [
        { arch: "arm64", kind: "exe" },
        { arch: "x64", kind: "exe" },
      ],
    );
    assert.deepEqual(
      collectReceiptUpdaterPlatforms(
        out,
        receipt,
        { updatePublicKey: env.ZHISHU_UPDATE_PUBLIC_KEY },
        { downloadBaseUrl: env.COS_PUBLIC_BASE_URL },
      ),
      latest.platforms,
    );
    assert.equal(latest.pub_date, "2026-10-29T03:00:00Z");
    for (const item of receipt.files)
      assert.equal(item.sha256, hashFile(join(out, item.name)));
    for (const item of website.files)
      assert.equal(item.sha256, hashFile(join(out, item.name)));
    assert.equal(
      latest.platforms["darwin-aarch64"].url,
      latest.platforms["darwin-x86_64"].url,
    );
    for (const item of Object.values(latest.platforms))
      assert.ok(item.url.startsWith(env.COS_PUBLIC_BASE_URL + "/v1.0.0/"));
    assert.equal(existsSync(join(out, "release.json")), true);
    const second = join(dir, "retry");
    prepareCosPayload(assets, "v1.0.0", second, env);
    assert.equal(
      hashFile(join(out, "release.json")),
      hashFile(join(second, "release.json")),
    );
  }));

test("tampered installers, missing platforms and wrong keys fail before preparing upload files", () => {
  for (const mutate of [
    (assets) =>
      writeFileSync(join(assets, "Zhishu-Ccs-v1.0.0-Windows.exe"), "tampered"),
    (assets) => rmSync(join(assets, "Zhishu-Ccs-v1.0.0-Windows-arm64.exe")),
    (assets) => rmSync(join(assets, "Zhishu-Ccs-v1.0.0-macOS.tar.gz.sig")),
  ])
    fixture((dir) => {
      const { assets, out, env } = payload(dir, mutate);
      assert.throws(() => prepareCosPayload(assets, "v1.0.0", out, env));
      assert.equal(existsSync(out), false);
    });
  fixture((dir) => {
    const { assets, out, env } = payload(dir);
    assert.throws(
      () =>
        prepareCosPayload(assets, "v1.0.0", out, {
          ...env,
          ZHISHU_UPDATE_PUBLIC_KEY: identity().key,
        }),
      /签名/,
    );
  });
});

test("unexpected files cannot leak into the public payload", () =>
  fixture((dir) => {
    const { assets, out, env } = payload(dir, (assets) =>
      writeFileSync(join(assets, "secret.key"), "private"),
    );
    assert.throws(
      () => prepareCosPayload(assets, "v1.0.0", out, env),
      /非预期/,
    );
    assert.equal(existsSync(out), false);
  }));

test("new releases reject legacy MSI, portable ZIP and paused Linux assets", () => {
  for (const name of [
    "Zhishu-Ccs-v1.0.0-Windows.msi",
    "Zhishu-Ccs-v1.0.0-Windows-Portable.zip",
    "Zhishu-Ccs-v1.0.0-Linux-arm64.deb",
  ])
    fixture((dir) => {
      const { assets, out, env } = payload(dir, (assets) =>
        writeFileSync(join(assets, name), "legacy"),
      );
      assert.throws(
        () => prepareCosPayload(assets, "v1.0.0", out, env),
        /非预期/,
      );
      assert.equal(existsSync(out), false);
    });
});

test("recovery verifies the original MSI/Linux payload using its historical receipt", () =>
  fixture((dir) => {
    const { assets, env } = payload(dir, () => {}, legacyTargets);
    const receipt = { version: "1.0.0", targets: Object.keys(legacyTargets) };
    const config = { updatePublicKey: env.ZHISHU_UPDATE_PUBLIC_KEY };
    const publish = { downloadBaseUrl: env.COS_PUBLIC_BASE_URL };
    const platforms = collectReceiptUpdaterPlatforms(
      assets,
      receipt,
      config,
      publish,
    );
    assert.equal(Object.keys(platforms).length, 10);
    assert.ok(platforms["windows-x86_64"].url.endsWith("-Windows.msi"));
    assert.ok(platforms["windows-aarch64"].url.endsWith("-Windows-arm64.msi"));
    assert.ok(platforms["linux-aarch64-deb"].url.endsWith("-Linux-arm64.deb"));
    for (const changed of [
      { ...receipt, packaging: "unknown" },
      { ...receipt, targets: receipt.targets.slice(1) },
      { ...receipt, packaging: "nsis-v1" },
    ])
      assert.throws(
        () => collectReceiptUpdaterPlatforms(assets, changed, config, publish),
        /打包策略/,
      );
    writeFileSync(
      join(assets, "Zhishu-Ccs-v1.0.0-Windows-arm64.msi"),
      "corrupted",
    );
    assert.throws(
      () => collectReceiptUpdaterPlatforms(assets, receipt, config, publish),
      /签名/,
    );
  }));
