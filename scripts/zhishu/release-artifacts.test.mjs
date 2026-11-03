import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  artifactNames,
  collectUpdaterPlatforms,
  targets,
  verifySignature,
  versionNumber,
} from "./release-artifacts.mjs";
import { validateReleaseConfig } from "./release-config.mjs";

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
function buildFixture(dir, id, version, key) {
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
      : [
          `Zhishu-Ccs-v${version}-Windows${target.arch === "arm64" ? "-arm64" : ""}.exe`,
        ];
  for (const name of names) {
    const bytes = Buffer.from(`fixture ${name}`);
    writeFileSync(join(out, name), bytes);
    if (!name.endsWith(".dmg") && !name.endsWith(".zip"))
      writeFileSync(join(out, name + ".sig"), key.signature(bytes));
  }
  return out;
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
  ]);
});
test("Windows releases use only NSIS EXE installers and Linux is not an active target", () => {
  assert.deepEqual(artifactNames("windows-x64", "1.0.0"), {
    exe: "Zhishu-Ccs-v1.0.0-Windows.exe",
  });
  assert.deepEqual(artifactNames("windows-arm64", "1.0.0"), {
    exe: "Zhishu-Ccs-v1.0.0-Windows-arm64.exe",
  });
  assert.equal(targets["windows-x64"].bundles, "nsis");
  assert.equal(targets["windows-arm64"].bundles, "nsis");
  for (const id of ["linux-x64", "linux-arm64"])
    assert.throws(() => artifactNames(id, "1.0.0"), /未知目标/);
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
test("build validation verifies every target in place without creating completion receipts or copies", () =>
  fixture((dir) => {
    const key = identity();
    const config = configFor(key.key);
    for (const id of Object.keys(targets)) {
      const input = buildFixture(dir, id, "4.0.20", key);
      const before = readdirSync(input).sort();
      const platforms = collectUpdaterPlatforms(
        input,
        [id],
        "4.0.20",
        config,
        publish,
      );
      assert.equal(Object.keys(platforms).length, id === "macos" ? 2 : 1);
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
    const input = buildFixture(dir, "windows-x64", "4.0.20", key);
    const installer = join(input, "Zhishu-Ccs-v4.0.20-Windows.exe");
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
    const input = buildFixture(dir, "windows-x64", "4.0.20", key);
    rmSync(join(input, "Zhishu-Ccs-v4.0.20-Windows.exe.sig"));
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
