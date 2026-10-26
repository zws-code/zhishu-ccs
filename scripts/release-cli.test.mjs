import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  assembleRelease,
  collectUpdaterPlatforms,
  hashFile,
  saveReceipt,
  targets,
  validateReceipt,
  verifySignature,
  versionNumber,
} from "./release-artifacts.mjs";
import { checkDownloadService, readPublishConfig } from "./release-cli.mjs";
import { validateReleaseConfig } from "./release-config.mjs";

const here = dirname(fileURLToPath(import.meta.url));
function fixture(fn) {
  const dir = mkdtempSync(join(tmpdir(), "zhishu-publish-test-"));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
function identity() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const raw = publicKey.export({ format: "der", type: "spki" }).subarray(-32);
  const id = Buffer.from("0011223344556677", "hex");
  const encoded = Buffer.concat([Buffer.from("Ed"), id, raw]);
  const key = Buffer.from(
    `untrusted comment: minisign public key: TEST\n${encoded.toString("base64")}\n`,
  ).toString("base64");
  const signature = (data, prehash = true) => {
    const bytes = sign(
      null,
      prehash ? createHash("blake2b512").update(data).digest() : data,
      privateKey,
    );
    const comment = "timestamp:123456\tfile:fixture";
    const global = sign(
      null,
      Buffer.concat([bytes, Buffer.from(comment)]),
      privateKey,
    );
    return Buffer.from(
      `untrusted comment: signature\n${Buffer.concat([Buffer.from(prehash ? "ED" : "Ed"), id, bytes]).toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`,
    ).toString("base64");
  };
  return { key, signature };
}
const configFor = (key) => ({
  platformApiUrl: "http://192.168.20.10:8080",
  platformWebUrl: "http://192.168.20.10:8080",
  platformAllowInsecureHttp: true,
  updateUrl: "http://192.168.20.10:8081/latest.json",
  updatePublicKey: key,
  updateAllowInsecureHttp: true,
});
const publish = {
  downloadBaseUrl: "http://192.168.20.10:8081",
  allowInsecureHttp: true,
};
function buildFixture(dir, id, version, config, key) {
  const out = join(dir, `v${version}-${id}`);
  mkdirSync(out, { recursive: true });
  const target = targets[id];
  const names =
    id === "macos"
      ? [
          `Zhishu-Ccs-v${version}-macOS.dmg`,
          `Zhishu-Ccs-v${version}-macOS.zip`,
          `Zhishu-Ccs-v${version}-macOS.tar.gz`,
        ]
      : target.os === "win32"
        ? [
            `Zhishu-Ccs-v${version}-Windows${target.arch === "arm64" ? "-arm64" : ""}.msi`,
          ]
        : ["AppImage", "deb", "rpm"].map(
            (ext) =>
              `Zhishu-Ccs-v${version}-Linux-${target.arch === "x64" ? "x86_64" : "arm64"}.${ext}`,
          );
  for (const name of names) {
    const bytes = Buffer.from(`fixture ${name}`);
    writeFileSync(join(out, name), bytes);
    if (!name.endsWith(".dmg") && !name.endsWith(".zip"))
      writeFileSync(join(out, name + ".sig"), key.signature(bytes));
  }
  saveReceipt(out, id, version, config);
  return out;
}
function assembled(dir, version = "4.0.20", ids = ["macos"], key = identity()) {
  const config = configFor(key.key);
  const inputs = ids.map((id) => buildFixture(dir, id, version, config, key));
  const out = join(dir, `.staging/${version}/payload`);
  assembleRelease(inputs, out, version, config, publish);
  return { out, config, inputs, key };
}

test("validates stable versions and all supported build targets", () => {
  assert.equal(versionNumber("v4.0.20"), "4.0.20");
  for (const value of [
    "../1.0.0",
    "01.0.0",
    "1.0.0-beta",
    "1.0",
    "1.2.3;touch x",
  ])
    assert.throws(() => versionNumber(value));
  assert.deepEqual(Object.keys(targets), [
    "macos",
    "windows-x64",
    "windows-arm64",
    "linux-x64",
    "linux-arm64",
  ]);
});
test("HTTP updater needs an explicit independent opt-in", () => {
  const config = configFor(identity().key);
  assert.deepEqual(validateReleaseConfig(config), config);
  assert.throws(
    () => validateReleaseConfig({ ...config, updateAllowInsecureHttp: false }),
    /HTTPS/,
  );
});
test("verifies prehashed and legacy minisign signatures; rejects tampering and wrong keys", () =>
  fixture((dir) => {
    const key = identity();
    const path = join(dir, "app");
    const bytes = Buffer.from("real payload");
    writeFileSync(path, bytes);
    for (const prehash of [true, false])
      verifySignature(path, key.signature(bytes, prehash), key.key);
    assert.throws(
      () => verifySignature(path, key.signature(bytes), identity().key),
      /签名/,
    );
    writeFileSync(path, "corrupted");
    assert.throws(
      () => verifySignature(path, key.signature(bytes), key.key),
      /签名/,
    );
  }));
test("build receipts reject changed files and a different production endpoint", () =>
  fixture((dir) => {
    const { inputs, config } = assembled(dir);
    assert.throws(
      () =>
        validateReceipt(inputs[0], "4.0.20", {
          ...config,
          platformApiUrl: "https://other.zhishu.cn",
        }),
      /不一致/,
    );
    writeFileSync(join(inputs[0], "Zhishu-Ccs-v4.0.20-macOS.dmg"), "changed");
    assert.throws(() => validateReceipt(inputs[0], "4.0.20", config), /损坏/);
  }));
test("assembles macOS, both Windows and both Linux architectures with real checksums and verified signatures", () =>
  fixture((dir) => {
    const { out } = assembled(dir, "4.0.20", Object.keys(targets));
    const website = JSON.parse(readFileSync(join(out, "client-release.json")));
    const latest = JSON.parse(readFileSync(join(out, "latest.json")));
    assert.equal(website.allowInsecureHttp, true);
    assert.equal(website.files.length, 10);
    assert.equal(Object.keys(latest.platforms).length, 10);
    assert.equal(
      latest.platforms["darwin-aarch64"].url,
      latest.platforms["darwin-x86_64"].url,
    );
    for (const file of website.files)
      assert.equal(file.sha256, hashFile(join(out, file.name)));
  }));
test("build validation verifies every target in place without creating completion receipts or copies", () =>
  fixture((dir) => {
    const key = identity();
    const config = configFor(key.key);
    for (const id of Object.keys(targets)) {
      const input = buildFixture(dir, id, "4.0.20", config, key);
      rmSync(join(input, "build.json"));
      const before = readdirSync(input).sort();
      const platforms = collectUpdaterPlatforms(
        input,
        [id],
        "4.0.20",
        config,
        publish,
      );
      assert.equal(
        Object.keys(platforms).length,
        id === "macos" ? 2 : id.startsWith("linux") ? 3 : 1,
      );
      assert.deepEqual(readdirSync(input).sort(), before);
      for (const entry of Object.values(platforms))
        assert.ok(entry.url.startsWith(`${publish.downloadBaseUrl}/v4.0.20/`));
      const installer = before.find((name) => !name.endsWith(".sig"));
      writeFileSync(join(input, installer), "");
      assert.throws(
        () => collectUpdaterPlatforms(input, [id], "4.0.20", config, publish),
        /安装包无效/,
      );
      assert.equal(existsSync(join(input, "build.json")), false);
    }
  }));
test("build validation rejects missing installers and bad signatures before completion", () =>
  fixture((dir) => {
    const key = identity();
    const config = configFor(key.key);
    const input = buildFixture(dir, "windows-x64", "4.0.20", config, key);
    const installer = join(input, "Zhishu-Ccs-v4.0.20-Windows.msi");
    writeFileSync(
      installer + ".sig",
      identity().signature(readFileSync(installer)),
    );
    assert.throws(
      () =>
        collectUpdaterPlatforms(
          input,
          ["windows-x64"],
          "4.0.20",
          config,
          publish,
        ),
      /签名/,
    );
    rmSync(installer);
    assert.throws(
      () =>
        collectUpdaterPlatforms(
          input,
          ["windows-x64"],
          "4.0.20",
          config,
          publish,
        ),
      /ENOENT/,
    );
  }));
test("a missing signature cannot become a published updater", () =>
  fixture((dir) => {
    const key = identity();
    const config = configFor(key.key);
    const input = buildFixture(dir, "windows-x64", "4.0.20", config, key);
    rmSync(join(input, "Zhishu-Ccs-v4.0.20-Windows.msi.sig"));
    saveReceipt(input, "windows-x64", "4.0.20", config);
    assert.throws(
      () =>
        assembleRelease([input], join(dir, "out"), "4.0.20", config, publish),
      /ENOENT/,
    );
  }));
test("publishing config rejects shell injection and path traversal before SSH", () =>
  fixture((dir) => {
    const config = {
      ...publish,
      server: "root@192.168.20.10",
      sshPort: 22,
      sshKey: "~/ssh-key",
      signingKey: "~/signing-key",
      remoteRoot: "/opt/zhishu-release-test",
      proxyManifest: "/opt/zhishu-proxy/data/public/client-release.json",
    };
    const path = join(dir, "release.publish.local.json");
    for (const change of [
      { server: "root@host;id" },
      { remoteRoot: "/opt/../etc" },
      { sshPort: 0 },
      { proxyManifest: "/etc/space name" },
    ]) {
      writeFileSync(path, JSON.stringify({ ...config, ...change }));
      assert.throws(() => readPublishConfig(dir));
    }
    writeFileSync(path, JSON.stringify(config));
    assert.equal(
      readPublishConfig(dir).downloadBaseUrl,
      publish.downloadBaseUrl,
    );
  }));
function server(mode, root, stage, proxy) {
  return spawnSync(
    "python3",
    [
      join(here, "../deploy/client-download/server.py"),
      mode,
      root,
      stage,
      proxy,
    ],
    { encoding: "utf8", env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" } },
  );
}
test(
  "server prepares files before switching manifests, safely retries, and refuses replacing a published version",
  {
    skip:
      process.platform === "win32"
        ? "POSIX server integration is checked on macOS/Linux"
        : false,
  },
  () =>
    fixture((dir) => {
      const { out } = assembled(dir);
      const proxy = join(dir, "proxy/client-release.json");
      mkdirSync(dirname(proxy));
      writeFileSync(proxy, '{"files":[]}');
      let result = server("prepare", dir, out, proxy);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(existsSync(join(dir, "current")), false);
      assert.equal(readFileSync(proxy, "utf8"), '{"files":[]}');
      const version = join(dir, "versions/v4.0.20");
      result = server("publish", dir, version, proxy);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(JSON.parse(readFileSync(proxy)).version, "4.0.20");
      assert.equal(server("publish", dir, version, proxy).status, 0);
      const other = join(dir, "other");
      mkdirSync(other);
      const changed = assembled(other);
      const stage = join(dir, ".staging/changed/payload");
      mkdirSync(stage, { recursive: true });
      for (const name of [
        "release.json",
        "SHA256SUMS",
        ...JSON.parse(
          readFileSync(join(changed.out, "release.json")),
        ).files.map((file) => file.name),
      ])
        copyFileSync(join(changed.out, name), join(stage, name));
      assert.notEqual(server("prepare", dir, stage, proxy).status, 0);
    }),
);
test(
  "server rejects upload corruption, dropped targets, and downgrade without moving the active release",
  {
    skip:
      process.platform === "win32"
        ? "POSIX server integration is checked on macOS/Linux"
        : false,
  },
  () =>
    fixture((dir) => {
      const first = assembled(dir, "4.0.20", ["macos", "windows-x64"]);
      const proxy = join(dir, "proxy/client-release.json");
      let result = server("publish", dir, first.out, proxy);
      assert.equal(result.status, 0, result.stderr);
      const second = assembled(dir, "4.0.21");
      result = server("publish", dir, second.out, proxy);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /Missing previously/);
      const older = assembled(dir, "4.0.19", ["macos", "windows-x64"]);
      result = server("publish", dir, older.out, proxy);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /older version/);
      assert.equal(JSON.parse(readFileSync(proxy)).version, "4.0.20");
      const broken = assembled(dir, "4.0.22", ["macos", "windows-x64"]);
      writeFileSync(
        join(broken.out, "Zhishu-Ccs-v4.0.22-Windows.msi"),
        "corrupt upload",
      );
      result = server("publish", dir, broken.out, proxy);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /Corrupt/);
      assert.equal(JSON.parse(readFileSync(proxy)).version, "4.0.20");
    }),
);

test("publication preflight rejects SPA fallback before any SSH upload", async () => {
  await assert.rejects(
    checkDownloadService(
      publish,
      async () => new Response("<html>SPA</html>", { status: 200 }),
    ),
    /未上传/,
  );
  await checkDownloadService(
    publish,
    async () => new Response("", { status: 404 }),
  );
});
